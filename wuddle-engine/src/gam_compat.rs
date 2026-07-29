//! Compatibility helpers for addons managed by GitAddonsManager (GAM).
//!
//! GAM treats every immediate `Interface/AddOns/*/.git` worktree as an addon
//! repository, follows the checked-out branch's upstream remote, and exposes
//! nested addon folders beside the worktree.  This module keeps those rules in
//! one place so Wuddle can consume GAM layouts without rewriting them.

use anyhow::{Context, Result};
use git2::{ObjectType, Repository, Tree};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use url::Url;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitIdentity {
    pub url: String,
    pub forge: String,
    pub host: String,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitRemote {
    pub name: String,
    pub url: String,
}

/// GAM renames displaced addon folders to `<name>.bak`, `<name>.bak.1`, ... .
/// These are backups, not active addons, and must never be auto-imported.
pub(crate) fn is_backup_folder_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let Some(pos) = lower.rfind(".bak") else {
        return false;
    };
    let suffix = &lower[pos + 4..];
    suffix.is_empty()
        || suffix
            .strip_prefix('.')
            .map(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or(false)
}

fn strip_dot_git(name: &str) -> String {
    let trimmed = name.trim().trim_end_matches('/');
    if trimmed
        .get(trimmed.len().saturating_sub(4)..)
        .map(|suffix| suffix.eq_ignore_ascii_case(".git"))
        .unwrap_or(false)
    {
        trimmed[..trimmed.len() - 4].to_string()
    } else {
        trimmed.to_string()
    }
}

fn forge_for_host(host: &str) -> &'static str {
    if host.eq_ignore_ascii_case("github.com") {
        "github"
    } else if host.eq_ignore_ascii_case("gitlab.com") {
        "gitlab"
    } else if host.eq_ignore_ascii_case("codeberg.org") {
        "codeberg"
    } else {
        "git"
    }
}

fn identity_from_parts(url: String, host: String, path: &str) -> Option<GitIdentity> {
    let path = path.trim().trim_matches('/');
    let mut segments = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let name = strip_dot_git(&segments.pop()?);
    if name.is_empty() {
        return None;
    }
    let owner = if segments.is_empty() {
        "_".to_string()
    } else {
        segments.join("/")
    };
    Some(GitIdentity {
        url,
        forge: forge_for_host(&host).to_string(),
        host,
        owner,
        name,
    })
}

/// Parse any clone URL accepted by git without guessing an unknown forge.
/// Full namespace paths are retained, which is important for self-hosted
/// GitLab and other servers with nested groups.
pub(crate) fn identity_from_remote(raw: &str) -> Option<GitIdentity> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // SCP-style SSH URL: git@example.org:group/subgroup/project.git
    if !raw.contains("://") {
        if let Some((user_host, path)) = raw.split_once(':') {
            if let Some((_, host)) = user_host.rsplit_once('@') {
                if !host.trim().is_empty() {
                    return identity_from_parts(raw.to_string(), host.trim().to_string(), path);
                }
            }
        }

        // Local and relative remotes are valid git remotes too.
        let path = Path::new(raw);
        let name = strip_dot_git(path.file_name()?.to_str()?);
        if name.is_empty() {
            return None;
        }
        let owner_path = path.parent().unwrap_or_else(|| Path::new("."));
        let owner = owner_path.to_string_lossy().replace('\\', "/");
        return Some(GitIdentity {
            url: raw.to_string(),
            forge: "git".to_string(),
            host: "local".to_string(),
            owner: if owner.is_empty() {
                ".".to_string()
            } else {
                owner
            },
            name,
        });
    }

    let parsed = Url::parse(raw).ok()?;
    if parsed.scheme().eq_ignore_ascii_case("file") {
        let path = parsed.to_file_path().ok()?;
        let name = strip_dot_git(path.file_name()?.to_str()?);
        let owner = path
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_string_lossy()
            .replace('\\', "/");
        return Some(GitIdentity {
            url: crate::url_safety::sanitize_remote_for_storage(parsed.as_str()),
            forge: "git".to_string(),
            host: "local".to_string(),
            owner,
            name,
        });
    }

    let host = parsed.host_str()?.to_string();
    let path = parsed.path().to_string();
    identity_from_parts(
        crate::url_safety::sanitize_remote_for_storage(parsed.as_str()),
        host,
        &path,
    )
}

/// Build stable local-only identity for a valid worktree with no configured
/// remote. It remains manageable, but update checks are intentionally disabled.
pub(crate) fn local_worktree_identity(worktree: &Path) -> GitIdentity {
    let folder = worktree
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("local-addon");
    let name = folder
        .strip_suffix(".repo")
        .or_else(|| folder.strip_suffix(".REPO"))
        .unwrap_or(folder)
        .to_string();
    let canonical = worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    GitIdentity {
        url: String::new(),
        forge: "git".to_string(),
        host: "local".to_string(),
        owner: format!("worktree-{:x}", digest)[..25].to_string(),
        name,
    }
}

fn remote_url(repo: &Repository, name: &str) -> Option<String> {
    let remote = repo.find_remote(name).ok()?;
    let url = remote.url().ok()?.trim();
    (!url.is_empty()).then(|| url.to_string())
}

fn remote_name_for_ref(repo: &Repository, refname: &str) -> Option<String> {
    if refname.starts_with("refs/heads/") {
        repo.branch_upstream_remote(refname)
            .ok()
            .and_then(|name| name.as_str().ok().map(str::to_string))
    } else if refname.starts_with("refs/remotes/") {
        repo.branch_remote_name(refname)
            .ok()
            .and_then(|name| name.as_str().ok().map(str::to_string))
    } else {
        None
    }
}

fn reflog_upstream_remote_name(repo: &Repository) -> Option<String> {
    let reflog = repo.reflog("HEAD").ok()?;
    for entry in reflog.iter() {
        let Some(message) = entry.message().ok().flatten() else {
            continue;
        };
        let Some(target) = message
            .strip_prefix("checkout: moving from ")
            .and_then(|message| message.split_once(" to ").map(|(_, target)| target))
        else {
            continue;
        };

        if let Ok(branch) = repo.find_branch(target, git2::BranchType::Local) {
            if let Ok(refname) = branch.get().name() {
                if let Some(remote) = remote_name_for_ref(repo, refname) {
                    return Some(remote);
                }
            }
        }
        if let Ok(branch) = repo.find_branch(target, git2::BranchType::Remote) {
            if let Ok(refname) = branch.get().name() {
                if let Some(remote) = remote_name_for_ref(repo, refname) {
                    return Some(remote);
                }
            }
        }
    }
    None
}

/// Return GAM's effective remote: checked-out branch upstream first, then
/// `origin`, then the first configured remote. No remote configuration is
/// changed by this lookup.
pub(crate) fn preferred_remote(repo: &Repository) -> Option<GitRemote> {
    if let Ok(head) = repo.head() {
        if let Ok(refname) = head.name() {
            let remote_name = remote_name_for_ref(repo, refname);
            if let Some(name) = remote_name {
                if let Some(url) = remote_url(repo, &name) {
                    return Some(GitRemote { name, url });
                }
            }
        }
    }

    if let Some(name) = reflog_upstream_remote_name(repo) {
        if let Some(url) = remote_url(repo, &name) {
            return Some(GitRemote { name, url });
        }
    }

    if let Some(url) = remote_url(repo, "origin") {
        return Some(GitRemote {
            name: "origin".to_string(),
            url,
        });
    }

    for name in repo
        .remotes()
        .ok()?
        .iter()
        .filter_map(|name| name.ok().flatten())
    {
        if let Some(url) = remote_url(repo, name) {
            return Some(GitRemote {
                name: name.to_string(),
                url,
            });
        }
    }
    None
}

pub(crate) fn identity_for_worktree(repo: &Repository, worktree: &Path) -> GitIdentity {
    preferred_remote(repo)
        .and_then(|remote| identity_from_remote(&remote.url))
        .unwrap_or_else(|| local_worktree_identity(worktree))
}

/// Resolve an exposed GAM addon and verify it belongs to the expected
/// worktree. Both GAM's symlink layout and its move fallback are valid.
pub(crate) fn exposed_addon_is_healthy(
    worktree: &Path,
    exposed: &Path,
    addon_name: &str,
    detect: impl Fn(&Path) -> Vec<String>,
) -> bool {
    if exposed == worktree {
        return detect(worktree)
            .iter()
            .any(|name| name.eq_ignore_ascii_case(addon_name));
    }

    let Ok(metadata) = std::fs::symlink_metadata(exposed) else {
        return false;
    };
    let is_link = metadata.file_type().is_symlink() || is_windows_directory_link(&metadata);
    if !is_link && !metadata.is_dir() {
        return false;
    }
    if !exposed.exists() {
        return false;
    }

    // A linked entry must point into this worktree, never into another addon.
    if is_link {
        let Ok(target) = exposed.canonicalize() else {
            return false;
        };
        let expected_root = worktree
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(worktree));
        if !target.starts_with(expected_root) {
            return false;
        }
    }

    detect(exposed)
        .iter()
        .any(|name| name.eq_ignore_ascii_case(addon_name))
}

fn case_insensitive_child(parent: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(parent)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
}

fn tree_matches_directory(repo: &Repository, tree: &Tree<'_>, directory: &Path) -> bool {
    tree.iter().all(|entry| {
        let Ok(name) = entry.name() else {
            return false;
        };
        let Some(path) = case_insensitive_child(directory, name) else {
            return false;
        };
        match entry.kind() {
            Some(ObjectType::Blob) if entry.filemode() == 0o120000 => false,
            Some(ObjectType::Blob) => {
                let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                    return false;
                };
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return false;
                }
                let Ok(contents) = std::fs::read(&path) else {
                    return false;
                };
                git2::Oid::hash_object(ObjectType::Blob, &contents)
                    .map(|oid| oid == entry.id())
                    .unwrap_or(false)
            }
            Some(ObjectType::Tree) => {
                let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                    return false;
                };
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return false;
                }
                repo.find_tree(entry.id())
                    .map(|child| tree_matches_directory(repo, &child, &path))
                    .unwrap_or(false)
            }
            _ => false,
        }
    })
}

fn tree_matches_directory_exact(repo: &Repository, tree: &Tree<'_>, directory: &Path) -> bool {
    if !tree_matches_directory(repo, tree, directory) {
        return false;
    }
    let expected = tree
        .iter()
        .filter_map(|entry| entry.name().ok().map(str::to_ascii_lowercase))
        .collect::<std::collections::HashSet<_>>();
    let actual = match std::fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|entry| entry.file_name().into_string().ok())
                    .map(|name| name.to_ascii_lowercase())
            })
            .collect::<std::collections::HashSet<_>>(),
        Err(_) => return false,
    };
    if actual != expected {
        return false;
    }

    tree.iter()
        .filter(|entry| entry.kind() == Some(ObjectType::Tree))
        .all(|entry| {
            let Ok(name) = entry.name() else {
                return false;
            };
            repo.find_tree(entry.id())
                .map(|child| tree_matches_directory_exact(repo, &child, &directory.join(name)))
                .unwrap_or(false)
        })
}

fn collect_addon_trees(
    repo: &Repository,
    tree: &Tree<'_>,
    addon_name: &str,
    matches: &mut Vec<git2::Oid>,
) {
    let defines_addon = tree.iter().any(|entry| {
        entry.kind() == Some(ObjectType::Blob)
            && entry
                .name()
                .ok()
                .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
                .map(|ext| ext.eq_ignore_ascii_case("toc"))
                .unwrap_or(false)
            && entry
                .name()
                .ok()
                .and_then(|name| Path::new(name).file_stem().and_then(|stem| stem.to_str()))
                .map(|stem| stem.eq_ignore_ascii_case(addon_name))
                .unwrap_or(false)
    });
    if defines_addon {
        matches.push(tree.id());
    }

    for entry in tree
        .iter()
        .filter(|entry| entry.kind() == Some(ObjectType::Tree))
    {
        if let Ok(child) = repo.find_tree(entry.id()) {
            collect_addon_trees(repo, &child, addon_name, matches);
        }
    }
}

fn collect_addon_tree_paths(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &Path,
    addon_name: &str,
    matches: &mut Vec<PathBuf>,
) {
    let defines_addon = tree.iter().any(|entry| {
        entry.kind() == Some(ObjectType::Blob)
            && entry
                .name()
                .ok()
                .and_then(|name| Path::new(name).extension().and_then(|ext| ext.to_str()))
                .map(|ext| ext.eq_ignore_ascii_case("toc"))
                .unwrap_or(false)
            && entry
                .name()
                .ok()
                .and_then(|name| Path::new(name).file_stem().and_then(|stem| stem.to_str()))
                .map(|stem| stem.eq_ignore_ascii_case(addon_name))
                .unwrap_or(false)
    });
    if defines_addon {
        matches.push(prefix.to_path_buf());
    }

    for entry in tree
        .iter()
        .filter(|entry| entry.kind() == Some(ObjectType::Tree))
    {
        let Ok(name) = entry.name() else {
            continue;
        };
        if let Ok(child) = repo.find_tree(entry.id()) {
            collect_addon_tree_paths(repo, &child, &prefix.join(name), addon_name, matches);
        }
    }
}

pub(crate) fn moved_addon_head_path(worktree: &Path, addon_name: &str) -> Option<PathBuf> {
    let repo = Repository::open(worktree).ok()?;
    let head_tree = repo.head().ok()?.peel_to_tree().ok()?;
    let mut matches = Vec::new();
    collect_addon_tree_paths(&repo, &head_tree, Path::new(""), addon_name, &mut matches);
    let [path] = matches.as_slice() else {
        return None;
    };
    Some(path.clone())
}

/// Verify GAM's real-folder fallback against the checked-out Git tree before
/// deleting it. Extra untracked files are preserved as part of the directory,
/// but every tracked file must still match HEAD. Ambiguous duplicate addon
/// definitions are deliberately rejected.
pub(crate) fn moved_addon_matches_head(worktree: &Path, exposed: &Path, addon_name: &str) -> bool {
    let Ok(repo) = Repository::open(worktree) else {
        return false;
    };
    let Ok(head_tree) = repo.head().and_then(|head| head.peel_to_tree()) else {
        return false;
    };
    let mut matches = Vec::new();
    collect_addon_trees(&repo, &head_tree, addon_name, &mut matches);
    let [tree_id] = matches.as_slice() else {
        return false;
    };
    repo.find_tree(*tree_id)
        .map(|tree| tree_matches_directory(&repo, &tree, exposed))
        .unwrap_or(false)
}

pub(crate) fn moved_addon_is_clean(worktree: &Path, exposed: &Path, addon_name: &str) -> bool {
    let Ok(repo) = Repository::open(worktree) else {
        return false;
    };
    let Ok(head_tree) = repo.head().and_then(|head| head.peel_to_tree()) else {
        return false;
    };
    let mut matches = Vec::new();
    collect_addon_trees(&repo, &head_tree, addon_name, &mut matches);
    let [tree_id] = matches.as_slice() else {
        return false;
    };
    repo.find_tree(*tree_id)
        .map(|tree| tree_matches_directory_exact(&repo, &tree, exposed))
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_windows_directory_link(metadata: &std::fs::Metadata) -> bool {
    metadata.is_dir() && (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

#[cfg(not(windows))]
fn is_windows_directory_link(_metadata: &std::fs::Metadata) -> bool {
    false
}

/// Expose a nested addon using GAM's platform behavior: relative symlink first
/// on Unix, real-folder move fallback everywhere, and the current GAM Windows
/// behavior of going directly to that fallback.
pub(crate) fn expose_module(
    worktree: &Path,
    relative_source: &str,
    destination: &Path,
) -> Result<crate::install::InstallRecord> {
    crate::install::link_addon_subfolder(worktree, relative_source, destination, cfg!(unix))
}

pub(crate) fn open_worktree(path: &Path) -> Result<Repository> {
    Repository::open(path).with_context(|| format!("open GAM worktree {:?}", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit_file(repo: &Repository, root: &Path) {
        std::fs::write(root.join("Addon.toc"), b"## Interface: 30300\n").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Wuddle Test", "test@example.invalid").unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();
    }

    #[test]
    fn recognizes_only_gam_backup_suffixes() {
        assert!(is_backup_folder_name("Addon.bak"));
        assert!(is_backup_folder_name("Addon.BAK.12"));
        assert!(!is_backup_folder_name("Addon.bak.old"));
        assert!(!is_backup_folder_name("bakery"));
    }

    #[test]
    fn generic_identity_preserves_nested_namespace() {
        let identity =
            identity_from_remote("https://forge.example/team/subgroup/project.git").unwrap();
        assert_eq!(identity.forge, "git");
        assert_eq!(identity.host, "forge.example");
        assert_eq!(identity.owner, "team/subgroup");
        assert_eq!(identity.name, "project");
    }

    #[test]
    fn scp_identity_preserves_nested_namespace() {
        let identity = identity_from_remote("git@forge.example:team/sub/project.git").unwrap();
        assert_eq!(identity.host, "forge.example");
        assert_eq!(identity.owner, "team/sub");
        assert_eq!(identity.name, "project");
    }

    #[test]
    fn strips_http_credentials_from_stored_identity() {
        let identity =
            identity_from_remote("https://user:secret@forge.example/team/project.git").unwrap();
        assert!(!identity.url.contains("user"));
        assert!(!identity.url.contains("secret"));
    }

    #[test]
    fn strips_query_and_fragment_from_stored_identity() {
        let identity = identity_from_remote(
            "https://forge.example/team/project.git?access_token=secret#branch",
        )
        .unwrap();
        assert_eq!(identity.url, "https://forge.example/team/project.git");
    }

    #[test]
    fn moved_folder_identity_requires_tracked_files_to_match_head() {
        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("Collection.repo");
        let exposed = temp.path().join("Module");
        let module = worktree.join("Module");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::write(module.join("Module.toc"), b"## Interface: 30300\n").unwrap();
        std::fs::write(module.join("Module.lua"), b"print('original')\n").unwrap();
        let repo = Repository::init(&worktree).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Wuddle Test", "test@example.invalid").unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();
        std::fs::rename(module, &exposed).unwrap();

        assert!(moved_addon_matches_head(&worktree, &exposed, "Module"));
        assert!(moved_addon_is_clean(&worktree, &exposed, "Module"));
        std::fs::write(exposed.join("user-note.txt"), b"keep me\n").unwrap();
        assert!(moved_addon_matches_head(&worktree, &exposed, "Module"));
        assert!(!moved_addon_is_clean(&worktree, &exposed, "Module"));
        std::fs::remove_file(exposed.join("user-note.txt")).unwrap();
        std::fs::write(exposed.join("Module.lua"), b"print('changed')\n").unwrap();
        assert!(!moved_addon_matches_head(&worktree, &exposed, "Module"));
        assert!(!moved_addon_is_clean(&worktree, &exposed, "Module"));
    }

    #[test]
    fn checked_out_branch_upstream_wins_over_origin() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        commit_file(&repo, temp.path());
        repo.remote("origin", "https://github.com/example/wrong.git")
            .unwrap();
        repo.remote("gam", "https://gitlab.com/example/right.git")
            .unwrap();
        let head_name = repo.head().unwrap().name().unwrap().to_string();
        let branch_name = head_name.strip_prefix("refs/heads/").unwrap();
        let mut config = repo.config().unwrap();
        config
            .set_str(&format!("branch.{branch_name}.remote"), "gam")
            .unwrap();
        config
            .set_str(&format!("branch.{branch_name}.merge"), "refs/heads/main")
            .unwrap();

        let selected = preferred_remote(&repo).unwrap();
        assert_eq!(selected.name, "gam");
        assert_eq!(selected.url, "https://gitlab.com/example/right.git");
    }

    #[test]
    fn detached_head_uses_last_branch_upstream_from_reflog() {
        let temp = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        commit_file(&repo, temp.path());
        repo.remote("origin", "https://github.com/example/wrong.git")
            .unwrap();
        repo.remote("gam", "https://gitlab.com/example/right.git")
            .unwrap();
        let head = repo.head().unwrap();
        let oid = head.target().unwrap();
        let branch_name = head.shorthand().unwrap().to_string();
        drop(head);
        let mut config = repo.config().unwrap();
        config
            .set_str(&format!("branch.{branch_name}.remote"), "gam")
            .unwrap();
        config
            .set_str(&format!("branch.{branch_name}.merge"), "refs/heads/main")
            .unwrap();
        drop(config);
        repo.set_head_detached(oid).unwrap();
        let mut reflog = repo.reflog("HEAD").unwrap();
        let signature = git2::Signature::now("Wuddle Test", "test@example.invalid").unwrap();
        reflog
            .append(
                oid,
                &signature,
                Some(&format!("checkout: moving from detached to {branch_name}")),
            )
            .unwrap();
        reflog.write().unwrap();

        let selected = preferred_remote(&repo).unwrap();
        assert_eq!(selected.name, "gam");
        assert_eq!(selected.url, "https://gitlab.com/example/right.git");
    }

    #[cfg(unix)]
    #[test]
    fn valid_gam_symlink_is_healthy_but_foreign_link_is_not() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("Collection.repo");
        let module = worktree.join("Module");
        let foreign = temp.path().join("Foreign");
        std::fs::create_dir_all(&module).unwrap();
        std::fs::create_dir_all(&foreign).unwrap();
        std::fs::write(module.join("Module.toc"), b"## Interface: 30300\n").unwrap();
        std::fs::write(foreign.join("Module.toc"), b"## Interface: 30300\n").unwrap();

        let exposed = temp.path().join("Module");
        symlink("./Collection.repo/Module", &exposed).unwrap();
        let detect = |path: &Path| {
            if path.join("Module.toc").is_file() {
                vec!["Module".to_string()]
            } else {
                Default::default()
            }
        };
        assert!(exposed_addon_is_healthy(
            &worktree, &exposed, "Module", detect
        ));

        std::fs::remove_file(&exposed).unwrap();
        symlink(&foreign, &exposed).unwrap();
        assert!(!exposed_addon_is_healthy(
            &worktree, &exposed, "Module", detect
        ));
    }
}
