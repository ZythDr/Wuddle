use anyhow::Result;
use url::Url;

/// Reject credentials supplied as part of a user-entered repository URL.
///
/// SSH usernames such as `git@host` are identifiers rather than secrets and
/// remain supported. Passwords in standard URLs and credential-like SCP
/// usernames are never accepted.
pub(crate) fn reject_embedded_credentials(raw: &str) -> Result<()> {
    let trimmed = raw.trim();
    if let Ok(parsed) = Url::parse(trimmed) {
        let web_userinfo = matches!(parsed.scheme(), "http" | "https")
            && (!parsed.username().is_empty() || parsed.password().is_some());
        if web_userinfo || parsed.password().is_some() {
            anyhow::bail!(
                "URLs containing credentials are not supported. Use a credential-free repository URL."
            );
        }
        if trimmed.contains("://")
            || matches!(parsed.scheme(), "http" | "https" | "ssh" | "git" | "file")
        {
            return Ok(());
        }
    }

    // SCP-style clone URLs normally look like git@example.org:group/repo.git.
    // A colon in the user portion is not valid as an SSH username here and is
    // most likely an attempted embedded password.
    if !trimmed.contains("://") {
        if let Some((user, host_path)) = trimmed.rsplit_once('@') {
            if user.contains(':') && host_path.contains(':') {
                anyhow::bail!(
                    "URLs containing credentials are not supported. Use a credential-free repository URL."
                );
            }
        }
    }
    Ok(())
}

/// Remove URL parts that are not a stable repository identity and may carry
/// secrets before a remote is persisted.
pub(crate) fn sanitize_remote_for_storage(raw: &str) -> String {
    let trimmed = raw.trim();
    // Preserve local paths and SCP-style Git remotes byte-for-byte. In
    // particular, URL parsers can mistake a Windows drive letter for a scheme.
    if !trimmed.contains("://") {
        return trimmed.to_string();
    }
    let Ok(mut parsed) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };

    parsed.set_query(None);
    parsed.set_fragment(None);
    if matches!(parsed.scheme(), "http" | "https") {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
    } else if parsed.password().is_some() {
        let _ = parsed.set_password(None);
    }
    parsed.to_string().trim_end_matches('/').to_string()
}

/// Produce a useful remote label without exposing credentials, signed query
/// parameters, fragments, or private local paths.
pub(crate) fn safe_remote_label(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Ok(parsed) = Url::parse(trimmed) {
        if parsed.scheme().eq_ignore_ascii_case("file") {
            return "local Git repository".to_string();
        }
        if let Some(host) = parsed.host_str() {
            return format!("Git repository on {}", host);
        }
    }

    if !trimmed.contains("://") {
        if let Some((user_host, _path)) = trimmed.split_once(':') {
            if let Some((_user, host)) = user_host.rsplit_once('@') {
                return format!("Git repository on {}", host);
            }
        }
        return "local Git repository".to_string();
    }

    "Git repository".to_string()
}

#[cfg(test)]
mod tests {
    use super::{reject_embedded_credentials, safe_remote_label, sanitize_remote_for_storage};

    #[test]
    fn rejects_web_credentials_but_allows_ssh_usernames() {
        assert!(reject_embedded_credentials("https://token@example.org/team/project.git").is_err());
        assert!(
            reject_embedded_credentials("https://user:secret@example.org/team/project.git")
                .is_err()
        );
        assert!(reject_embedded_credentials("ssh://git@example.org/team/project.git").is_ok());
        assert!(reject_embedded_credentials("git@example.org:team/project.git").is_ok());
        assert!(reject_embedded_credentials("user:secret@example.org:team/project.git").is_err());
    }

    #[test]
    fn storage_identity_drops_web_userinfo_query_and_fragment() {
        assert_eq!(
            sanitize_remote_for_storage(
                "https://user:secret@example.org/team/project.git?token=secret#ref"
            ),
            "https://example.org/team/project.git"
        );
        assert_eq!(
            sanitize_remote_for_storage("ssh://git@example.org/team/project.git?x=1#ref"),
            "ssh://git@example.org/team/project.git"
        );
        assert_eq!(
            sanitize_remote_for_storage(r"C:\Games\addon.git"),
            r"C:\Games\addon.git"
        );
        assert_eq!(
            sanitize_remote_for_storage("git@example.org:team/project.git"),
            "git@example.org:team/project.git"
        );
    }

    #[test]
    fn safe_labels_hide_local_paths_and_scp_users() {
        assert_eq!(
            safe_remote_label("/home/alice/private/project.git"),
            "local Git repository"
        );
        assert_eq!(
            safe_remote_label("git@example.org:team/project.git"),
            "Git repository on example.org"
        );
        assert_eq!(
            safe_remote_label("https://user:secret@example.org/private/project.git?token=x"),
            "Git repository on example.org"
        );
    }
}
