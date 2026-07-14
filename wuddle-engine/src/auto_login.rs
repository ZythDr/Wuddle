//! Secure, feature-gated storage and argument preparation for compatible WoW clients.
//!
//! This module deliberately does not own profile persistence, UI state, databases, or
//! process launching. It keeps the credential boundary small: callers identify an
//! account, and a [`PreparedArguments`] value can decorate a command without exposing
//! ordinary credential strings.

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

const KEYCHAIN_SERVICE: &str = "wuddle";
const KEYCHAIN_PREFIX: &str = "wow_autologin";
const KEYCHAIN_TIMEOUT: Duration = Duration::from_millis(2500);
const PAYLOAD_VERSION: u8 = 1;
pub const CUSTOM_ARGUMENTS_PLACEHOLDER: &str = "{autologin_args}";

#[derive(Clone)]
pub struct SecretText(SecretString);

impl SecretText {
    pub fn new(value: String) -> Self {
        Self(SecretString::from(value))
    }

    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn is_empty(&self) -> bool {
        self.expose().is_empty()
    }
}

impl Default for SecretText {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl fmt::Debug for SecretText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretText([REDACTED])")
    }
}

impl From<String> for SecretText {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Error)]
pub enum AutoLoginError {
    #[error("Auto-login {field} is required.")]
    MissingField { field: &'static str },
    #[error("Auto-login account labels must be unique within an instance.")]
    DuplicateLabel,
    #[error("The auto-login account identifier is invalid.")]
    InvalidAccountId,
    #[error("Auto-login {field} contains an unsupported null character.")]
    InvalidField { field: &'static str },
    #[error("The selected auto-login account was not found in secure storage.")]
    NotFound,
    #[error("The saved auto-login credential is invalid or from an unsupported version.")]
    CorruptEntry,
    #[error("The system credential vault timed out while {operation}.")]
    VaultTimeout { operation: &'static str },
    #[error("The system credential vault is unavailable: {0}")]
    VaultUnavailable(String),
    #[error("The credential vault did not return the value that was just saved.")]
    VerificationFailed,
    #[error("The credential vault could not restore the previous value after a failed save.")]
    RollbackFailed,
    #[error("Custom auto-login requires exactly one {CUSTOM_ARGUMENTS_PLACEHOLDER} token.")]
    CustomPlaceholder,
    #[error("Could not encode the auto-login credential.")]
    Encode,
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(String);

impl AccountId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, AutoLoginError> {
        let value = value.into();
        uuid::Uuid::parse_str(&value).map_err(|_| AutoLoginError::InvalidAccountId)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AccountId").field(&self.0).finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRef {
    pub id: AccountId,
    pub label: String,
}

impl AccountRef {
    pub fn new(label: impl Into<String>) -> Result<Self, AutoLoginError> {
        let label = normalize_required(label.into(), "account label")?;
        Ok(Self {
            id: AccountId::new(),
            label,
        })
    }

    pub fn validate_unique_label(
        label: &str,
        accounts: &[AccountRef],
        editing: Option<&AccountId>,
    ) -> Result<String, AutoLoginError> {
        let label = normalize_required(label.to_string(), "account label")?;
        if accounts.iter().any(|account| {
            editing != Some(&account.id) && account.label.eq_ignore_ascii_case(&label)
        }) {
            return Err(AutoLoginError::DuplicateLabel);
        }
        Ok(label)
    }
}

pub struct CredentialInput {
    login: SecretString,
    password: Option<SecretString>,
    realmlist: SecretString,
    realm_name: SecretString,
}

impl fmt::Debug for CredentialInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CredentialInput([REDACTED])")
    }
}

impl CredentialInput {
    pub fn new(
        login: String,
        password: Option<String>,
        realmlist: String,
        realm_name: String,
    ) -> Result<Self, AutoLoginError> {
        let login = normalize_required(login, "login")?;
        reject_nul(&login, "login")?;
        if let Some(password) = password.as_deref() {
            if password.is_empty() {
                return Err(AutoLoginError::MissingField { field: "password" });
            }
            reject_nul(password, "password")?;
        }
        reject_nul(&realmlist, "realmlist")?;
        reject_nul(&realm_name, "realm name")?;
        Ok(Self {
            login: SecretString::from(login),
            password: password.map(SecretString::from),
            realmlist: SecretString::from(realmlist.trim().to_string()),
            realm_name: SecretString::from(realm_name.trim().to_string()),
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AccountDetails {
    pub login: String,
    pub realmlist: String,
    pub realm_name: String,
}

impl fmt::Debug for AccountDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountDetails([REDACTED])")
    }
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct StoredCredential {
    version: u8,
    login: String,
    password: String,
    realmlist: String,
    realm_name: String,
}

impl fmt::Debug for StoredCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoredCredential([REDACTED])")
    }
}

impl StoredCredential {
    fn validate(self) -> Result<Self, AutoLoginError> {
        if self.version != PAYLOAD_VERSION || self.login.is_empty() || self.password.is_empty() {
            return Err(AutoLoginError::CorruptEntry);
        }
        Ok(self)
    }
}

pub trait CredentialBackend: Clone + Send + Sync + 'static {
    fn get(&self, account: &str) -> Result<Option<String>, AutoLoginError>;
    fn set(&self, account: &str, secret: &str) -> Result<(), AutoLoginError>;
    fn delete(&self, account: &str) -> Result<(), AutoLoginError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCredentialBackend;

impl CredentialBackend for SystemCredentialBackend {
    fn get(&self, account: &str) -> Result<Option<String>, AutoLoginError> {
        let account = account.to_string();
        keychain_call("reading a credential", move || {
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account).map_err(vault_error)?;
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(error) => Err(vault_error(error)),
            }
        })
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), AutoLoginError> {
        let account = account.to_string();
        let secret = Zeroizing::new(secret.to_string());
        keychain_call("saving a credential", move || {
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account).map_err(vault_error)?;
            entry.set_password(&secret).map_err(vault_error)
        })
    }

    fn delete(&self, account: &str) -> Result<(), AutoLoginError> {
        let account = account.to_string();
        keychain_call("deleting a credential", move || {
            let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &account).map_err(vault_error)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(vault_error(error)),
            }
        })
    }
}

fn keychain_call<T, F>(operation: &'static str, call: F) -> Result<T, AutoLoginError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AutoLoginError> + Send + 'static,
{
    keychain_call_with_timeout(operation, KEYCHAIN_TIMEOUT, call)
}

fn keychain_call_with_timeout<T, F>(
    operation: &'static str,
    timeout: Duration,
    call: F,
) -> Result<T, AutoLoginError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AutoLoginError> + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(call());
    });
    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(AutoLoginError::VaultTimeout { operation }),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(AutoLoginError::VaultUnavailable(
            "credential worker stopped unexpectedly".to_string(),
        )),
    }
}

fn vault_error(error: keyring::Error) -> AutoLoginError {
    AutoLoginError::VaultUnavailable(error.to_string())
}

#[derive(Debug, Clone)]
pub struct AutoLoginService<B = SystemCredentialBackend> {
    backend: B,
}

impl Default for AutoLoginService<SystemCredentialBackend> {
    fn default() -> Self {
        Self::system()
    }
}

impl AutoLoginService<SystemCredentialBackend> {
    pub fn system() -> Self {
        Self {
            backend: SystemCredentialBackend,
        }
    }
}

impl<B: CredentialBackend> AutoLoginService<B> {
    pub fn with_backend(backend: B) -> Self {
        Self { backend }
    }

    pub fn save_account(
        &self,
        profile_id: &str,
        account_id: &AccountId,
        input: CredentialInput,
    ) -> Result<(), AutoLoginError> {
        let key = credential_key(profile_id, account_id)?;
        let existing = self.read_stored_by_key(&key)?;
        let previous_encoded = existing
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| AutoLoginError::Encode)?
            .map(Zeroizing::new);
        let password = match input.password {
            Some(password) => password.expose_secret().to_string(),
            None => existing
                .as_ref()
                .map(|credential| credential.password.clone())
                .ok_or(AutoLoginError::MissingField { field: "password" })?,
        };
        let credential = StoredCredential {
            version: PAYLOAD_VERSION,
            login: input.login.expose_secret().to_string(),
            password,
            realmlist: input.realmlist.expose_secret().to_string(),
            realm_name: input.realm_name.expose_secret().to_string(),
        };
        let encoded =
            Zeroizing::new(serde_json::to_string(&credential).map_err(|_| AutoLoginError::Encode)?);
        self.backend.set(&key, &encoded)?;
        let verification = self.backend.get(&key);
        if matches!(&verification, Ok(Some(value)) if value == encoded.as_str()) {
            return Ok(());
        }

        let rollback = match previous_encoded {
            Some(previous) => self.backend.set(&key, &previous),
            None => self.backend.delete(&key),
        };
        if rollback.is_err() {
            return Err(AutoLoginError::RollbackFailed);
        }
        match verification {
            Err(error) => Err(error),
            _ => Err(AutoLoginError::VerificationFailed),
        }
    }

    pub fn load_account_details(
        &self,
        profile_id: &str,
        account_id: &AccountId,
    ) -> Result<AccountDetails, AutoLoginError> {
        let credential = self.read_stored(profile_id, account_id)?;
        Ok(AccountDetails {
            login: credential.login.clone(),
            realmlist: credential.realmlist.clone(),
            realm_name: credential.realm_name.clone(),
        })
    }

    pub fn delete_account(
        &self,
        profile_id: &str,
        account_id: &AccountId,
    ) -> Result<(), AutoLoginError> {
        let key = credential_key(profile_id, account_id)?;
        self.backend.delete(&key)?;
        if self.backend.get(&key)?.is_some() {
            return Err(AutoLoginError::VerificationFailed);
        }
        Ok(())
    }

    pub fn prepare_arguments(
        &self,
        profile_id: &str,
        account_id: &AccountId,
    ) -> Result<PreparedArguments, AutoLoginError> {
        let credential = self.read_stored(profile_id, account_id)?;
        PreparedArguments::from_stored(&credential)
    }

    fn read_stored(
        &self,
        profile_id: &str,
        account_id: &AccountId,
    ) -> Result<StoredCredential, AutoLoginError> {
        self.read_stored_by_key(&credential_key(profile_id, account_id)?)?
            .ok_or(AutoLoginError::NotFound)
    }

    fn read_stored_by_key(&self, key: &str) -> Result<Option<StoredCredential>, AutoLoginError> {
        let Some(encoded) = self.backend.get(key)? else {
            return Ok(None);
        };
        let encoded = Zeroizing::new(encoded);
        let credential = serde_json::from_str::<StoredCredential>(&encoded)
            .map_err(|_| AutoLoginError::CorruptEntry)?
            .validate()?;
        Ok(Some(credential))
    }
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PreparedArguments {
    values: Vec<SecretString>,
}

impl fmt::Debug for PreparedArguments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PreparedArguments([REDACTED])")
    }
}

impl PreparedArguments {
    fn from_stored(credential: &StoredCredential) -> Result<Self, AutoLoginError> {
        let mut values = vec![
            SecretString::from("-login".to_string()),
            SecretString::from(credential.login.clone()),
            SecretString::from("-password".to_string()),
            SecretString::from(credential.password.clone()),
        ];
        if !credential.realmlist.is_empty() {
            values.push(SecretString::from("-realmlist".to_string()));
            values.push(SecretString::from(credential.realmlist.clone()));
        }
        if !credential.realm_name.is_empty() {
            values.push(SecretString::from("-realmname".to_string()));
            values.push(SecretString::from(credential.realm_name.clone()));
        }
        Ok(Self { values })
    }

    pub fn append_to_command(&self, command: &mut Command) {
        for value in &self.values {
            command.arg(value.expose_secret());
        }
    }

    pub fn append_custom_command(
        &self,
        command: &mut Command,
        base_arguments: &[String],
    ) -> Result<(), AutoLoginError> {
        let placeholders = base_arguments
            .iter()
            .filter(|argument| argument.as_str() == CUSTOM_ARGUMENTS_PLACEHOLDER)
            .count();
        if placeholders != 1 {
            return Err(AutoLoginError::CustomPlaceholder);
        }
        for argument in base_arguments {
            if argument == CUSTOM_ARGUMENTS_PLACEHOLDER {
                self.append_to_command(command);
            } else {
                command.arg(argument);
            }
        }
        Ok(())
    }
}

pub fn append_manual_custom_arguments(command: &mut Command, base_arguments: &[String]) {
    for argument in base_arguments {
        if argument != CUSTOM_ARGUMENTS_PLACEHOLDER {
            command.arg(argument);
        }
    }
}

fn credential_key(profile_id: &str, account_id: &AccountId) -> Result<String, AutoLoginError> {
    let profile_id = profile_id.trim();
    if profile_id.is_empty()
        || profile_id.contains([':', '\0'])
        || uuid::Uuid::parse_str(account_id.as_str()).is_err()
    {
        return Err(AutoLoginError::InvalidAccountId);
    }
    Ok(format!(
        "{KEYCHAIN_PREFIX}:{profile_id}:{}",
        account_id.as_str()
    ))
}

fn normalize_required(value: String, field: &'static str) -> Result<String, AutoLoginError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(AutoLoginError::MissingField { field })
    } else {
        reject_nul(&value, field)?;
        Ok(value)
    }
}

fn reject_nul(value: &str, field: &'static str) -> Result<(), AutoLoginError> {
    if value.contains('\0') {
        Err(AutoLoginError::InvalidField { field })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Default)]
    struct MemoryBackend {
        values: Arc<Mutex<HashMap<String, String>>>,
        fail_reads: bool,
        fail_writes: bool,
        fail_deletes: bool,
        corrupt_next_write: Arc<AtomicBool>,
        ignore_deletes: Arc<AtomicBool>,
    }

    impl CredentialBackend for MemoryBackend {
        fn get(&self, account: &str) -> Result<Option<String>, AutoLoginError> {
            if self.fail_reads {
                return Err(AutoLoginError::VaultUnavailable("test failure".into()));
            }
            Ok(self.values.lock().unwrap().get(account).cloned())
        }

        fn set(&self, account: &str, secret: &str) -> Result<(), AutoLoginError> {
            if self.fail_writes {
                return Err(AutoLoginError::VaultUnavailable("test failure".into()));
            }
            let secret = if self.corrupt_next_write.swap(false, Ordering::SeqCst) {
                "{corrupt"
            } else {
                secret
            };
            self.values
                .lock()
                .unwrap()
                .insert(account.to_string(), secret.to_string());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), AutoLoginError> {
            if self.fail_deletes {
                return Err(AutoLoginError::VaultUnavailable("test failure".into()));
            }
            if !self.ignore_deletes.load(Ordering::SeqCst) {
                self.values.lock().unwrap().remove(account);
            }
            Ok(())
        }
    }

    fn input(password: Option<&str>) -> CredentialInput {
        CredentialInput::new(
            "user@example.test".into(),
            password.map(str::to_string),
            "logon.example.test".into(),
            "Example Realm".into(),
        )
        .unwrap()
    }

    #[test]
    fn account_id_and_label_validation() {
        let first = AccountRef::new("Main").unwrap();
        assert!(AccountId::parse(first.id.as_str()).is_ok());
        assert!(matches!(
            AccountRef::validate_unique_label("main", &[first], None),
            Err(AutoLoginError::DuplicateLabel)
        ));
    }

    #[test]
    fn save_read_update_without_password_and_delete() {
        let backend = MemoryBackend::default();
        let service = AutoLoginService::with_backend(backend.clone());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("secret")))
            .unwrap();

        let details = service.load_account_details("profile", &account).unwrap();
        assert_eq!(details.login, "user@example.test");

        let updated =
            CredentialInput::new("new-login".into(), None, String::new(), String::new()).unwrap();
        service.save_account("profile", &account, updated).unwrap();
        let prepared = service.prepare_arguments("profile", &account).unwrap();
        let mut command = Command::new("wow.exe");
        prepared.append_to_command(&mut command);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args, ["-login", "new-login", "-password", "secret"]);

        service.delete_account("profile", &account).unwrap();
        assert!(matches!(
            service.prepare_arguments("profile", &account),
            Err(AutoLoginError::NotFound)
        ));
    }

    #[test]
    fn new_account_requires_password() {
        let service = AutoLoginService::with_backend(MemoryBackend::default());
        let result = service.save_account("profile", &AccountId::new(), input(None));
        assert!(matches!(
            result,
            Err(AutoLoginError::MissingField { field: "password" })
        ));
    }

    #[test]
    fn arguments_preserve_values_and_optional_order() {
        let service = AutoLoginService::with_backend(MemoryBackend::default());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("space secret!")))
            .unwrap();
        let prepared = service.prepare_arguments("profile", &account).unwrap();
        let mut command = Command::new("wow.exe");
        prepared.append_to_command(&mut command);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "-login",
                "user@example.test",
                "-password",
                "space secret!",
                "-realmlist",
                "logon.example.test",
                "-realmname",
                "Example Realm"
            ]
        );
    }

    #[test]
    fn custom_placeholder_is_exactly_once() {
        let service = AutoLoginService::with_backend(MemoryBackend::default());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("secret")))
            .unwrap();
        let prepared = service.prepare_arguments("profile", &account).unwrap();
        let mut command = Command::new("wrapper");
        prepared
            .append_custom_command(
                &mut command,
                &[
                    "--before".into(),
                    CUSTOM_ARGUMENTS_PLACEHOLDER.into(),
                    "--after".into(),
                ],
            )
            .unwrap();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert_eq!(args.first().map(String::as_str), Some("--before"));
        assert_eq!(args.last().map(String::as_str), Some("--after"));

        let mut invalid = Command::new("wrapper");
        assert!(matches!(
            prepared.append_custom_command(&mut invalid, &["--no-placeholder".into()]),
            Err(AutoLoginError::CustomPlaceholder)
        ));
    }

    #[test]
    fn debug_output_is_redacted() {
        let service = AutoLoginService::with_backend(MemoryBackend::default());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("very-secret")))
            .unwrap();
        let prepared = service.prepare_arguments("profile", &account).unwrap();
        let debug = format!("{prepared:?}");
        assert!(!debug.contains("very-secret"));
        assert!(debug.contains("REDACTED"));
    }

    #[test]
    fn backend_failures_are_propagated_without_secrets() {
        let backend = MemoryBackend {
            fail_writes: true,
            ..MemoryBackend::default()
        };
        let service = AutoLoginService::with_backend(backend);
        let error = service
            .save_account("profile", &AccountId::new(), input(Some("do-not-print")))
            .unwrap_err();
        assert!(!error.to_string().contains("do-not-print"));
    }

    #[test]
    fn corrupt_entries_are_rejected() {
        let backend = MemoryBackend::default();
        let account = AccountId::new();
        let key = credential_key("profile", &account).unwrap();
        backend
            .values
            .lock()
            .unwrap()
            .insert(key, "{not-json".to_string());
        let service = AutoLoginService::with_backend(backend);
        assert!(matches!(
            service.prepare_arguments("profile", &account),
            Err(AutoLoginError::CorruptEntry)
        ));
    }

    #[test]
    fn failed_replacement_restores_the_previous_credential() {
        let backend = MemoryBackend::default();
        let service = AutoLoginService::with_backend(backend.clone());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("old-secret")))
            .unwrap();

        backend.corrupt_next_write.store(true, Ordering::SeqCst);
        let replacement = CredentialInput::new(
            "new-login".into(),
            Some("new-secret".into()),
            "".into(),
            "".into(),
        )
        .unwrap();
        assert!(matches!(
            service.save_account("profile", &account, replacement),
            Err(AutoLoginError::VerificationFailed)
        ));

        let details = service.load_account_details("profile", &account).unwrap();
        assert_eq!(details.login, "user@example.test");
        let prepared = service.prepare_arguments("profile", &account).unwrap();
        let mut command = Command::new("wow.exe");
        prepared.append_to_command(&mut command);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(args.iter().any(|argument| argument == "old-secret"));
        assert!(!args.iter().any(|argument| argument == "new-secret"));
    }

    #[test]
    fn failed_new_save_removes_the_unverified_credential() {
        let backend = MemoryBackend::default();
        backend.corrupt_next_write.store(true, Ordering::SeqCst);
        let service = AutoLoginService::with_backend(backend.clone());
        let account = AccountId::new();
        assert!(matches!(
            service.save_account("profile", &account, input(Some("secret"))),
            Err(AutoLoginError::VerificationFailed)
        ));
        assert!(backend.values.lock().unwrap().is_empty());
    }

    #[test]
    fn deletion_is_verified() {
        let backend = MemoryBackend::default();
        let service = AutoLoginService::with_backend(backend.clone());
        let account = AccountId::new();
        service
            .save_account("profile", &account, input(Some("secret")))
            .unwrap();
        backend.ignore_deletes.store(true, Ordering::SeqCst);
        assert!(matches!(
            service.delete_account("profile", &account),
            Err(AutoLoginError::VerificationFailed)
        ));
    }

    #[test]
    fn credential_worker_timeout_is_typed() {
        let error =
            keychain_call_with_timeout("testing a credential", Duration::from_millis(5), || {
                std::thread::sleep(Duration::from_millis(30));
                Ok(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            AutoLoginError::VaultTimeout {
                operation: "testing a credential"
            }
        ));
    }

    #[test]
    fn prepared_arguments_have_a_zeroize_on_drop_contract() {
        fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<PreparedArguments>();
    }
}
