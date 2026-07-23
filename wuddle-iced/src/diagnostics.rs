//! Persistent, privacy-safe diagnostics for support reports.

use chrono::{Local, SecondsFormat};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

const ACTIVE_LOG: &str = "wuddle.log";
const MAX_LOG_FILES: usize = 5;
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

static LOGGER: OnceLock<Arc<DiagnosticLogger>> = OnceLock::new();
static INIT_ERROR: OnceLock<String> = OnceLock::new();

struct FileState {
    file: Option<File>,
    bytes_written: u64,
}

struct DiagnosticLogger {
    log_dir: PathBuf,
    state: Mutex<FileState>,
    verbose: AtomicBool,
    private_values: RwLock<Vec<(String, String)>>,
}

pub fn init(verbose: bool) -> Result<(), String> {
    let result = init_inner(verbose);
    if let Err(error) = &result {
        let _ = INIT_ERROR.set(error.clone());
    }
    result
}

fn init_inner(verbose: bool) -> Result<(), String> {
    if let Some(logger) = LOGGER.get() {
        logger.verbose.store(verbose, Ordering::Release);
        return Ok(());
    }

    let app_dir = crate::settings::app_dir()?;
    let log_dir = app_dir.join("logs");
    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("Could not create diagnostics directory: {error}"))?;
    rotate_files(&log_dir)?;
    let active_path = log_dir.join(ACTIVE_LOG);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&active_path)
        .map_err(|error| format!("Could not open diagnostic log: {error}"))?;
    let bytes_written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
    let logger = Arc::new(DiagnosticLogger {
        log_dir,
        state: Mutex::new(FileState {
            file: Some(file),
            bytes_written,
        }),
        verbose: AtomicBool::new(verbose),
        private_values: RwLock::new(Vec::new()),
    });

    logger.register_private_path(&app_dir, "<WUDDLE_DATA>");
    if let Some(home) = dirs::home_dir() {
        logger.register_private_path(&home, "~");
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            logger.register_private_path(parent, "<WUDDLE_INSTALL>");
        }
    }

    LOGGER
        .set(Arc::clone(&logger))
        .map_err(|_| "Diagnostic logger was already initialized".to_string())?;
    let sink_logger = Arc::clone(&logger);
    wuddle_engine::diagnostics::set_sink(Some(Arc::new(move |level, target, message| {
        let level = match level {
            wuddle_engine::diagnostics::DiagnosticLevel::Debug => "DEBUG",
            wuddle_engine::diagnostics::DiagnosticLevel::Trace => "TRACE",
        };
        sink_logger.write(level, target, message, true);
    })));

    logger.write(
        "INFO",
        "diagnostics",
        &format!(
            "Diagnostic session started; version={}; os={}; arch={}; verbose={verbose}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        false,
    );
    Ok(())
}

pub fn init_error() -> Option<&'static str> {
    INIT_ERROR.get().map(String::as_str)
}

pub fn set_verbose(verbose: bool) {
    if let Some(logger) = LOGGER.get() {
        logger.verbose.store(verbose, Ordering::Release);
        logger.write(
            "INFO",
            "diagnostics",
            if verbose {
                "Verbose diagnostic logging enabled"
            } else {
                "Verbose diagnostic logging disabled"
            },
            false,
        );
    }
}

pub fn is_verbose() -> bool {
    LOGGER
        .get()
        .map(|logger| logger.verbose.load(Ordering::Acquire))
        .unwrap_or(false)
}

pub fn register_private_path(path: impl AsRef<Path>, replacement: &'static str) {
    if let Some(logger) = LOGGER.get() {
        logger.register_private_path(path.as_ref(), replacement);
    }
}

pub fn register_private_value(value: &str, replacement: &'static str) {
    if let Some(logger) = LOGGER.get() {
        logger.register_private_value(value, replacement);
    }
}

pub fn register_settings_paths(settings: &crate::settings::AppSettings) {
    for profile in &settings.profiles {
        register_private_value(&profile.id, "<PROFILE_ID>");
        register_private_value(&profile.name, "<PROFILE_NAME>");
        if !profile.wow_dir.trim().is_empty() {
            register_private_path(&profile.wow_dir, "<GAME_PATH>");
        }
        if !profile.working_dir.trim().is_empty() {
            register_private_path(&profile.working_dir, "<WORKING_DIR>");
        }
        #[cfg(feature = "auto-login")]
        for account in &profile.auto_login_accounts {
            register_private_value(&account.label, "<ACCOUNT_LABEL>");
        }
    }
}

pub fn write_app(level: crate::LogLevel, message: &str) {
    let level = match level {
        crate::LogLevel::Info => "INFO",
        crate::LogLevel::Api => "API",
        crate::LogLevel::Error => "ERROR",
    };
    if let Some(logger) = LOGGER.get() {
        logger.write(level, "ui", message, false);
    }
}

pub fn debug(target: &'static str, message: impl AsRef<str>) {
    if let Some(logger) = LOGGER.get() {
        logger.write("DEBUG", target, message.as_ref(), true);
    }
}

pub fn trace(target: &'static str, message: impl AsRef<str>) {
    if let Some(logger) = LOGGER.get() {
        logger.write("TRACE", target, message.as_ref(), true);
    }
}

pub fn write_system(level: &'static str, target: &'static str, message: &str) {
    if let Some(logger) = LOGGER.get() {
        logger.write(level, target, message, false);
    }
}

pub struct OperationGuard {
    name: &'static str,
    started: std::time::Instant,
}

impl OperationGuard {
    pub fn new(name: &'static str) -> Self {
        debug("service", format!("{name}: started"));
        Self {
            name,
            started: std::time::Instant::now(),
        }
    }
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        debug(
            "service",
            format!(
                "{}: finished in {} ms",
                self.name,
                self.started.elapsed().as_millis()
            ),
        );
    }
}

pub fn export_bundle(target: &Path, summary: &str) -> Result<(), String> {
    let logger = LOGGER
        .get()
        .ok_or_else(|| "Diagnostic logging is unavailable".to_string())?;
    logger.export_bundle(target, summary)
}

pub fn default_export_filename() -> String {
    format!(
        "wuddle-diagnostics-{}.zip",
        Local::now().format("%Y%m%d-%H%M%S")
    )
}

pub fn build_summary(app: &crate::App) -> String {
    let active_profile = app.active_profile();
    let launch_method = active_profile
        .map(|profile| profile.launch_method.as_str())
        .unwrap_or("unknown");
    let auto_login_enabled = active_profile
        .map(|profile| profile.auto_login_enabled)
        .unwrap_or(false);
    let enabled_repos = app.repos.iter().filter(|repo| repo.enabled).count();
    let mods = app
        .repos
        .iter()
        .filter(|repo| crate::service::is_mod(repo))
        .count();
    let addons = app.repos.len().saturating_sub(mods);
    let busy_state = app.busy_summary().unwrap_or_else(|| "idle".to_string());
    let active_update_progress = crate::service::active_update_check_progress();
    let active_update_stages = if active_update_progress.is_empty() {
        "none".to_string()
    } else {
        active_update_progress
            .iter()
            .map(|progress| format!("{}:{:?}", progress.repo_id, progress.stage))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        concat!(
            "Wuddle diagnostic summary\n",
            "version={}\n",
            "os={}\n",
            "architecture={}\n",
            "verbose={}\n",
            "portable_mode={}\n",
            "update_channel={:?}\n",
            "ui_scale_mode={:?}\n",
            "profile_count={}\n",
            "repository_count={}\n",
            "enabled_repository_count={}\n",
            "mod_count={}\n",
            "addon_count={}\n",
            "active_launch_method={}\n",
            "active_auto_login_enabled={}\n",
            "automatic_update_checks={}\n",
            "conserve_github_api={}\n",
            "github_authenticated={}\n",
            "symlink_installs={}\n",
            "xattr_comments={}\n",
            "busy_state={}\n",
            "active_update_check_count={}\n",
            "active_update_check_stages={}\n"
        ),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        app.verbose_diagnostics,
        crate::settings::portable_mode_enabled(),
        app.update_channel,
        app.ui_scale_mode,
        app.profiles.len(),
        app.repos.len(),
        enabled_repos,
        mods,
        addons,
        launch_method,
        auto_login_enabled,
        app.opt_auto_check,
        app.opt_conserve_github_api,
        wuddle_engine::github_token().is_some(),
        app.opt_symlinks,
        app.opt_xattr,
        busy_state,
        active_update_progress.len(),
        active_update_stages,
    )
}

pub fn sanitize_text(text: &str) -> String {
    LOGGER
        .get()
        .map(|logger| logger.sanitize(text))
        .unwrap_or_else(|| {
            redact_url_secrets(redact_named_values(redact_prefixed_tokens(
                text.to_string(),
            )))
        })
}

impl DiagnosticLogger {
    fn register_private_path(&self, path: &Path, replacement: &'static str) {
        let value = path
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();
        if value.is_empty() {
            return;
        }
        self.register_private_value(&value, replacement);
    }

    fn register_private_value(&self, value: &str, replacement: &'static str) {
        let value = value.trim();
        if value.len() < 3 {
            return;
        }
        if let Ok(mut values) = self.private_values.write() {
            if !values.iter().any(|(existing, _)| existing == value) {
                values.push((value.to_string(), replacement.to_string()));
                values.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
            }
        }
    }

    fn write(&self, level: &str, target: &str, message: &str, verbose_only: bool) {
        if verbose_only && !self.verbose.load(Ordering::Acquire) {
            return;
        }
        let message = self.sanitize(message).replace(['\r', '\n'], "\\n");
        let line = format!(
            "{} [{level}] [{target}] {message}\n",
            Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
        );
        if let Ok(mut state) = self.state.lock() {
            if state.bytes_written.saturating_add(line.len() as u64) > MAX_LOG_BYTES
                && self.rotate_locked(&mut state).is_err()
            {
                return;
            }
            if let Some(file) = state.file.as_mut() {
                if file.write_all(line.as_bytes()).is_ok() {
                    state.bytes_written = state.bytes_written.saturating_add(line.len() as u64);
                }
            }
        }
    }

    fn rotate_locked(&self, state: &mut FileState) -> Result<(), String> {
        if let Some(mut file) = state.file.take() {
            let _ = file.flush();
        }
        rotate_files(&self.log_dir)?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_dir.join(ACTIVE_LOG))
            .map_err(|error| format!("Could not rotate diagnostic log: {error}"))?;
        state.file = Some(file);
        state.bytes_written = 0;
        Ok(())
    }

    fn sanitize(&self, message: &str) -> String {
        let mut sanitized = message.to_string();
        if let Ok(values) = self.private_values.read() {
            for (private, replacement) in values.iter() {
                sanitized = replace_case_insensitive(&sanitized, private, replacement);
                let slash_variant = private.replace('\\', "/");
                if slash_variant != *private {
                    sanitized = replace_case_insensitive(&sanitized, &slash_variant, replacement);
                }
            }
        }
        sanitized = redact_prefixed_tokens(sanitized);
        sanitized = redact_named_values(sanitized);
        redact_url_secrets(sanitized)
    }

    fn export_bundle(&self, target: &Path, summary: &str) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Diagnostic log lock was poisoned".to_string())?;
        if let Some(file) = state.file.as_mut() {
            file.flush()
                .map_err(|error| format!("Could not flush diagnostic log: {error}"))?;
        }

        let output = File::create(target)
            .map_err(|error| format!("Could not create diagnostic bundle: {error}"))?;
        let mut archive = zip::ZipWriter::new(output);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for index in 0..MAX_LOG_FILES {
            let file_name = log_file_name(index);
            let path = self.log_dir.join(&file_name);
            if !path.is_file() {
                continue;
            }
            let mut contents = String::new();
            File::open(&path)
                .and_then(|mut file| file.read_to_string(&mut contents))
                .map_err(|error| format!("Could not read diagnostic log: {error}"))?;
            archive
                .start_file(format!("logs/{file_name}"), options)
                .map_err(|error| format!("Could not write diagnostic bundle: {error}"))?;
            archive
                .write_all(self.sanitize(&contents).as_bytes())
                .map_err(|error| format!("Could not write diagnostic bundle: {error}"))?;
        }

        archive
            .start_file("diagnostics.txt", options)
            .map_err(|error| format!("Could not write diagnostic summary: {error}"))?;
        archive
            .write_all(self.sanitize(summary).as_bytes())
            .map_err(|error| format!("Could not write diagnostic summary: {error}"))?;
        archive
            .start_file("PRIVACY.txt", options)
            .map_err(|error| format!("Could not write privacy notice: {error}"))?;
        archive
            .write_all(b"This bundle intentionally excludes credentials, tokens, command arguments, request headers, database contents, raw settings, and account/profile names. Private path prefixes are replaced before they are written to the logs.\n")
            .map_err(|error| format!("Could not write privacy notice: {error}"))?;
        archive
            .finish()
            .map_err(|error| format!("Could not finish diagnostic bundle: {error}"))?;
        Ok(())
    }
}

fn rotate_files(log_dir: &Path) -> Result<(), String> {
    let oldest = log_dir.join(log_file_name(MAX_LOG_FILES - 1));
    if oldest.exists() {
        fs::remove_file(&oldest)
            .map_err(|error| format!("Could not prune old diagnostic log: {error}"))?;
    }
    for index in (0..MAX_LOG_FILES - 1).rev() {
        let source = log_dir.join(log_file_name(index));
        if !source.exists() {
            continue;
        }
        let destination = log_dir.join(log_file_name(index + 1));
        fs::rename(&source, &destination)
            .map_err(|error| format!("Could not rotate diagnostic log: {error}"))?;
    }
    Ok(())
}

fn log_file_name(index: usize) -> String {
    if index == 0 {
        ACTIVE_LOG.to_string()
    } else {
        format!("wuddle.{index}.log")
    }
}

fn replace_case_insensitive(input: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return input.to_string();
    }
    let lower_input = input.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative) = lower_input[cursor..].find(&lower_needle) {
        let start = cursor + relative;
        result.push_str(&input[cursor..start]);
        result.push_str(replacement);
        cursor = start + needle.len();
    }
    result.push_str(&input[cursor..]);
    result
}

fn redact_prefixed_tokens(mut input: String) -> String {
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"] {
        loop {
            let lower = input.to_ascii_lowercase();
            let Some(start) = lower.find(prefix) else {
                break;
            };
            let end = input[start..]
                .char_indices()
                .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
                .last()
                .map(|(offset, ch)| start + offset + ch.len_utf8())
                .unwrap_or(start + prefix.len());
            input.replace_range(start..end, "<REDACTED_TOKEN>");
        }
    }
    input
}

fn redact_named_values(mut input: String) -> String {
    for key in ["password", "token", "secret", "authorization"] {
        let mut search_from = 0;
        loop {
            let lower = input.to_ascii_lowercase();
            let Some(relative) = lower[search_from..].find(key) else {
                break;
            };
            let key_start = search_from + relative;
            let mut cursor = key_start + key.len();
            while input
                .as_bytes()
                .get(cursor)
                .is_some_and(u8::is_ascii_whitespace)
                || input.as_bytes().get(cursor) == Some(&b'"')
            {
                cursor += 1;
            }
            if !matches!(input.as_bytes().get(cursor), Some(b'=') | Some(b':')) {
                search_from = cursor;
                continue;
            }
            cursor += 1;
            while input
                .as_bytes()
                .get(cursor)
                .is_some_and(u8::is_ascii_whitespace)
            {
                cursor += 1;
            }
            let quoted = input.as_bytes().get(cursor) == Some(&b'"');
            if quoted {
                cursor += 1;
            }
            let value_start = cursor;
            while let Some(byte) = input.as_bytes().get(cursor) {
                if (quoted && *byte == b'"') || (!quoted && matches!(*byte, b',' | b';')) {
                    break;
                }
                cursor += 1;
            }
            if value_start < cursor {
                input.replace_range(value_start..cursor, "<REDACTED>");
                search_from = value_start + "<REDACTED>".len();
            } else {
                search_from = cursor;
            }
        }
    }
    input
}

fn redact_url_secrets(mut input: String) -> String {
    let mut search_from = 0;
    loop {
        let lower = input.to_ascii_lowercase();
        let next_http = lower[search_from..].find("http://");
        let next_https = lower[search_from..].find("https://");
        let Some(relative) = [next_http, next_https].into_iter().flatten().min() else {
            break;
        };
        let start = search_from + relative;
        let end = input[start..]
            .find(char::is_whitespace)
            .map(|offset| start + offset)
            .unwrap_or(input.len());
        let mut url = input[start..end].to_string();
        if let Some(scheme_end) = url.find("://").map(|index| index + 3) {
            if let Some(at) = url[scheme_end..].find('@') {
                url.replace_range(scheme_end..scheme_end + at + 1, "<REDACTED_USERINFO>@");
            }
        }
        if let Some(secret_start) = url.find(['?', '#']) {
            let trailing = url
                .chars()
                .rev()
                .take_while(|ch| matches!(ch, ')' | ']' | '}' | ',' | ';'))
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            url.truncate(secret_start);
            url.push_str("?<REDACTED_QUERY>");
            url.push_str(&trailing);
        }
        input.replace_range(start..end, &url);
        search_from = start + url.len();
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizer_removes_tokens_credentials_queries_and_private_paths() {
        let temp = tempfile::tempdir().unwrap();
        let logger = DiagnosticLogger {
            log_dir: temp.path().to_path_buf(),
            state: Mutex::new(FileState {
                file: None,
                bytes_written: 0,
            }),
            verbose: AtomicBool::new(true),
            private_values: RwLock::new(vec![(
                "/home/alice/Games/WoW".to_string(),
                "<GAME_PATH>".to_string(),
            )]),
        };
        let sanitized = logger.sanitize(
            r#"path=/home/alice/Games/WoW token=ghp_abcdefghijklmnopqrstuvwxyz password=Pass123 authorization: Bearer private-value url=https://user:pass@example.com/repo?access_token=abc"#,
        );
        assert!(!sanitized.contains("alice"));
        assert!(!sanitized.contains("ghp_"));
        assert!(!sanitized.contains("Pass123"));
        assert!(!sanitized.contains("private-value"));
        assert!(!sanitized.contains("user:pass"));
        assert!(!sanitized.contains("access_token"));
        assert!(sanitized.contains("<GAME_PATH>"));
    }

    #[test]
    fn export_resanitizes_log_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join(ACTIVE_LOG),
            "token=ghp_should_not_escape\npassword=Pass123\n",
        )
        .unwrap();
        let logger = DiagnosticLogger {
            log_dir: temp.path().to_path_buf(),
            state: Mutex::new(FileState {
                file: None,
                bytes_written: 0,
            }),
            verbose: AtomicBool::new(true),
            private_values: RwLock::new(Vec::new()),
        };
        let bundle = temp.path().join("diagnostics.zip");
        logger.export_bundle(&bundle, "safe summary").unwrap();

        let mut archive = zip::ZipArchive::new(File::open(bundle).unwrap()).unwrap();
        let mut log = String::new();
        archive
            .by_name("logs/wuddle.log")
            .unwrap()
            .read_to_string(&mut log)
            .unwrap();
        assert!(!log.contains("ghp_should_not_escape"));
        assert!(!log.contains("Pass123"));
        assert!(log.contains("<REDACTED"));
    }
}
