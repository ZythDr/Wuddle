use anyhow::{anyhow, Context, Result};
use git2::{
    build::{CheckoutBuilder, RepoBuilder},
    Cred, Direction, FetchOptions, Oid, RemoteCallbacks, Repository,
};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::{Duration, Instant};
use tempfile::tempdir;

use crate::gam_compat;

const SERVER_CONNECT_TIMEOUT_MS: i32 = 5_000;
const SERVER_IO_TIMEOUT_MS: i32 = 15_000;

/// Configure libgit2's process-wide network deadlines.
///
/// libgit2 exposes these as C globals, so Wuddle calls this once at process
/// startup, before Iced/Tokio can create worker threads. The callbacks below
/// add a per-operation deadline and cancellation signal on top of these
/// transport-level limits.
pub(crate) fn initialize_transport_timeouts() -> Result<()> {
    static INITIALIZED: OnceLock<Result<(), String>> = OnceLock::new();
    let result = INITIALIZED.get_or_init(|| {
        // SAFETY: Wuddle's binaries call this at the very beginning of `main`,
        // before either runtime starts any worker threads. OnceLock also
        // prevents concurrent writes to libgit2's process-global options.
        unsafe {
            git2::opts::set_server_connect_timeout_in_milliseconds(SERVER_CONNECT_TIMEOUT_MS)
                .map_err(|error| error.to_string())?;
            git2::opts::set_server_timeout_in_milliseconds(SERVER_IO_TIMEOUT_MS)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    });
    result.clone().map_err(anyhow::Error::msg)
}

fn git_failure(action: &str, url: &str, error: &git2::Error) -> anyhow::Error {
    anyhow!(
        "{} {} failed ({:?}/{:?})",
        action,
        crate::url_safety::safe_remote_label(url),
        error.class(),
        error.code()
    )
}

#[derive(Debug, Clone)]
pub struct GitHeadState {
    pub oid: String,
    pub short_oid: String,
    pub branch: String,
    pub remote_ref: String,
}

fn short_oid(oid: Oid) -> String {
    oid.to_string().chars().take(10).collect()
}

fn sanitize_fs_component(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for ch in v.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_".to_string()
    } else {
        out
    }
}

#[derive(Clone)]
struct RemoteOperationControl {
    deadline: Option<Instant>,
    cancelled: Option<Arc<AtomicBool>>,
}

impl RemoteOperationControl {
    fn bounded(timeout: Duration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline: Some(Instant::now() + timeout),
            cancelled: Some(cancelled),
        }
    }

    fn may_continue(&self) -> bool {
        !self
            .cancelled
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
            && !self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn check(&self) -> Result<()> {
        if self.may_continue() {
            Ok(())
        } else {
            anyhow::bail!("Git remote operation was cancelled or timed out")
        }
    }
}

fn remote_callbacks(control: Option<RemoteOperationControl>) -> RemoteCallbacks<'static> {
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|_url, username_from_url, allowed| {
        if allowed.is_ssh_key() {
            if let Some(user) = username_from_url {
                return Cred::ssh_key_from_agent(user);
            }
        }
        if allowed.is_username() {
            return Cred::username(username_from_url.unwrap_or("git"));
        }
        Cred::default()
    });
    if let Some(control) = control {
        let transfer_control = control.clone();
        cb.transfer_progress(move |_| transfer_control.may_continue());
        let sideband_control = control.clone();
        cb.sideband_progress(move |_| sideband_control.may_continue());
        let tips_control = control.clone();
        cb.update_tips(move |_, _, _| tips_control.may_continue());
    }
    cb
}

fn git_url_candidates(url: &str) -> Vec<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let base = trimmed.trim_end_matches('/').to_string();
    let mut out = Vec::new();
    let add_dot_git =
        (base.starts_with("https://") || base.starts_with("http://") || base.starts_with("git@"))
            && !base.ends_with(".git");
    if add_dot_git {
        out.push(format!("{base}.git"));
    }
    out.push(base.clone());
    out
}

#[derive(Debug, Clone)]
struct RemoteRefInfo {
    name: String,
    symref_target: Option<String>,
    oid: Oid,
}

#[derive(Debug, Clone)]
struct ConfiguredRemote {
    name: String,
    url: String,
    pushurl: Option<String>,
    fetch_refspecs: Vec<String>,
    push_refspecs: Vec<String>,
}

fn configured_remotes(repo: &Repository) -> Result<Vec<ConfiguredRemote>> {
    let names = repo.remotes().context("list configured Git remotes")?;
    let mut configured = Vec::new();
    for name in names.iter().filter_map(|name| name.ok().flatten()) {
        let remote = repo
            .find_remote(name)
            .with_context(|| format!("open configured Git remote {name}"))?;
        let url = remote
            .url()
            .context("read configured Git remote URL")?
            .to_string();
        let pushurl = remote
            .pushurl()
            .context("read configured Git push URL")?
            .map(str::to_string);
        let mut fetch_refspecs = Vec::new();
        let mut push_refspecs = Vec::new();
        for refspec in remote.refspecs() {
            let value = refspec
                .str()
                .context("read configured Git refspec")?
                .to_string();
            match refspec.direction() {
                Direction::Fetch => fetch_refspecs.push(value),
                Direction::Push => push_refspecs.push(value),
            }
        }
        configured.push(ConfiguredRemote {
            name: name.to_string(),
            url,
            pushurl,
            fetch_refspecs,
            push_refspecs,
        });
    }
    Ok(configured)
}

fn restore_remote_configuration(
    source: &Repository,
    staged: &Repository,
    branch: &str,
) -> Result<()> {
    let preferred = gam_compat::preferred_remote(source);
    let configured = configured_remotes(source)?;
    if configured.is_empty() {
        return Ok(());
    }

    let staged_names = staged
        .remotes()
        .context("list staged Git remotes")?
        .iter()
        .filter_map(|name| name.ok().flatten())
        .map(str::to_string)
        .collect::<Vec<_>>();
    for name in staged_names {
        staged
            .remote_delete(&name)
            .with_context(|| format!("remove temporary staged Git remote {name}"))?;
    }

    for remote in &configured {
        staged
            .remote(&remote.name, &remote.url)
            .with_context(|| format!("restore staged Git remote {}", remote.name))?;
        if let Some(pushurl) = remote.pushurl.as_deref() {
            staged
                .remote_set_pushurl(&remote.name, Some(pushurl))
                .with_context(|| format!("restore staged Git push URL for {}", remote.name))?;
        }

        // `Repository::remote` supplies the standard fetch refspec. Preserve
        // additional custom refspecs without adding that default twice.
        let default_fetch = format!("+refs/heads/*:refs/remotes/{}/*", remote.name);
        for refspec in &remote.fetch_refspecs {
            if refspec != &default_fetch {
                staged
                    .remote_add_fetch(&remote.name, refspec)
                    .with_context(|| {
                        format!("restore staged Git fetch refspec for {}", remote.name)
                    })?;
            }
        }
        for refspec in &remote.push_refspecs {
            staged
                .remote_add_push(&remote.name, refspec)
                .with_context(|| format!("restore staged Git push refspec for {}", remote.name))?;
        }
    }

    if let Some(preferred) = preferred {
        let mut config = staged.config().context("open staged Git configuration")?;
        config
            .set_str(&format!("branch.{branch}.remote"), &preferred.name)
            .context("restore staged Git branch remote")?;
        config
            .set_str(
                &format!("branch.{branch}.merge"),
                &format!("refs/heads/{branch}"),
            )
            .context("restore staged Git branch merge ref")?;
    }
    Ok(())
}

fn remote_refs_for_url(
    url: &str,
    control: Option<&RemoteOperationControl>,
) -> Result<Vec<RemoteRefInfo>> {
    if let Some(control) = control {
        control.check()?;
    }
    let tmp = tempdir().context("create temporary git dir")?;
    let bare_repo = Repository::init_bare(tmp.path()).context("init temporary bare repo")?;
    let mut remote = bare_repo
        .remote_anonymous(url)
        .map_err(|error| git_failure("Create remote", url, &error))?;

    // Try credential-aware connect first (works for both public and private remotes),
    // then fall back to plain anonymous fetch if needed.
    let auth_res = remote
        .connect_auth(
            Direction::Fetch,
            Some(remote_callbacks(control.cloned())),
            None,
        )
        .map(|_| ());
    if auth_res.is_err() {
        if let Some(control) = control {
            control.check()?;
        }
        remote
            .connect(Direction::Fetch)
            .map_err(|error| git_failure("Connect to remote", url, &error))?;
    }
    if let Some(control) = control {
        control.check()?;
    }
    let refs = remote
        .list()
        .map_err(|error| git_failure("List remote refs for", url, &error))?
        .iter()
        .map(|h| RemoteRefInfo {
            name: h.name().to_string(),
            symref_target: h.symref_target().map(|s| s.to_string()),
            oid: h.oid(),
        })
        .collect::<Vec<_>>();

    remote
        .disconnect()
        .map_err(|error| git_failure("Disconnect remote", url, &error))?;
    Ok(refs)
}

fn choose_remote_head_for_url(
    url: &str,
    preferred_branch: Option<&str>,
    control: Option<&RemoteOperationControl>,
) -> Result<GitHeadState> {
    let refs = remote_refs_for_url(url, control)?;

    let preferred_ref = preferred_branch
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(|b| format!("refs/heads/{b}"));
    let mut remote_ref = preferred_ref
        .as_deref()
        .and_then(|rf| refs.iter().find(|h| h.name == rf).map(|h| h.name.clone()));
    let mut oid = remote_ref
        .as_deref()
        .and_then(|rf| refs.iter().find(|h| h.name == rf).map(|h| h.oid));

    if remote_ref.is_none() {
        remote_ref = refs
            .iter()
            .find(|h| h.name == "HEAD")
            .and_then(|h| h.symref_target.clone());
        oid = remote_ref
            .as_deref()
            .and_then(|rf| refs.iter().find(|h| h.name == rf).map(|h| h.oid));
    }

    if remote_ref.is_none() {
        for cand in ["refs/heads/main", "refs/heads/master"] {
            if let Some(h) = refs.iter().find(|h| h.name == cand) {
                remote_ref = Some(cand.to_string());
                oid = Some(h.oid);
                break;
            }
        }
    }

    if remote_ref.is_none() || oid.is_none() {
        if let Some(h) = refs
            .iter()
            .find(|h| h.name.starts_with("refs/heads/") && !h.oid.is_zero())
        {
            remote_ref = Some(h.name.clone());
            oid = Some(h.oid);
        }
    }

    let remote_ref = remote_ref.ok_or_else(|| anyhow!("Could not detect remote HEAD ref"))?;
    let oid = oid.ok_or_else(|| anyhow!("Could not detect remote HEAD commit"))?;
    let branch = remote_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(remote_ref.as_str())
        .to_string();
    Ok(GitHeadState {
        oid: oid.to_string(),
        short_oid: short_oid(oid),
        branch,
        remote_ref,
    })
}

fn choose_remote_head_with_url(
    url: &str,
    preferred_branch: Option<&str>,
    control: Option<&RemoteOperationControl>,
) -> Result<(GitHeadState, String)> {
    let candidates = git_url_candidates(url);
    if candidates.is_empty() {
        anyhow::bail!("Git URL is empty");
    }

    let mut last_err = None;
    for candidate in candidates {
        if let Some(control) = control {
            control.check()?;
        }
        match choose_remote_head_for_url(&candidate, preferred_branch, control) {
            Ok(state) => return Ok((state, candidate)),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        anyhow!(
            "Could not connect to {}",
            crate::url_safety::safe_remote_label(url)
        )
    }))
}

fn choose_remote_head_for_branch(
    url: &str,
    preferred_branch: Option<&str>,
    control: Option<&RemoteOperationControl>,
) -> Result<GitHeadState> {
    choose_remote_head_with_url(url, preferred_branch, control).map(|(state, _)| state)
}

fn remote_branches_for_url(
    url: &str,
    control: Option<&RemoteOperationControl>,
) -> Result<Vec<String>> {
    let refs = remote_refs_for_url(url, control)?;
    let mut branches = refs
        .into_iter()
        .filter_map(|r| r.name.strip_prefix("refs/heads/").map(|s| s.to_string()))
        .collect::<Vec<_>>();
    branches.sort_by_key(|b| b.to_ascii_lowercase());
    branches.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(branches)
}

pub fn local_head(path: &Path) -> Result<Option<GitHeadState>> {
    if !path.exists() {
        return Ok(None);
    }
    let repo = Repository::open(path).with_context(|| {
        format!(
            "Addon folder exists but is not a git repository: {}",
            path.display()
        )
    })?;
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    let oid = match head.target() {
        Some(v) => v,
        None => return Ok(None),
    };
    let remote_ref = head.name().unwrap_or("HEAD").to_string();
    let branch = remote_ref
        .strip_prefix("refs/heads/")
        .unwrap_or(remote_ref.as_str())
        .to_string();
    Ok(Some(GitHeadState {
        oid: oid.to_string(),
        short_oid: short_oid(oid),
        branch,
        remote_ref,
    }))
}

fn ensure_git_repo(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    match Repository::open(path) {
        Ok(_) => Ok(true),
        Err(_) => anyhow::bail!(
            "Addon folder exists but is not a git repository: {}",
            path.display()
        ),
    }
}

fn clone_repo(url: &str, path: &Path, branch: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let plain_res = {
        let mut builder = RepoBuilder::new();
        if !branch.trim().is_empty() {
            builder.branch(branch);
        }
        builder.clone(url, path)
    };
    if plain_res.is_ok() {
        return Ok(());
    }

    if path.exists() {
        let _ = std::fs::remove_dir_all(path);
    }

    let first_err = plain_res
        .err()
        .ok_or_else(|| anyhow!("unexpected clone state"))?;
    let mut fo = FetchOptions::new();
    fo.remote_callbacks(remote_callbacks(None));
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fo);
    if !branch.trim().is_empty() {
        builder.branch(branch);
    }
    builder.clone(url, path).map_err(|error| {
        anyhow!(
            "{}; unauthenticated attempt also failed ({:?}/{:?})",
            git_failure("Clone", url, &error),
            first_err.class(),
            first_err.code()
        )
    })?;
    Ok(())
}

fn sync_existing_repo(url: &str, path: &Path, remote: &GitHeadState) -> Result<()> {
    let repo = Repository::open(path).with_context(|| format!("open repo {}", path.display()))?;
    let configured = gam_compat::preferred_remote(&repo);
    let remote_name = configured
        .as_ref()
        .map(|configured| configured.name.as_str())
        .unwrap_or("origin")
        .to_string();
    let mut git_remote = match repo.find_remote(&remote_name) {
        Ok(remote) => remote,
        Err(_) => repo
            .remote(&remote_name, url)
            .map_err(|error| git_failure("Add Git remote", url, &error))?,
    };

    let plain_fetch = git_remote
        .fetch(&[remote.remote_ref.as_str()], None, None)
        .or_else(|_| git_remote.fetch(&[remote.branch.as_str()], None, None));
    if let Err(first_err) = plain_fetch {
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(remote_callbacks(None));
        git_remote
            .fetch(&[remote.remote_ref.as_str()], Some(&mut fo), None)
            .or_else(|_| git_remote.fetch(&[remote.branch.as_str()], Some(&mut fo), None))
            .map_err(|error| {
                anyhow!(
                    "{}; unauthenticated attempt also failed ({:?}/{:?})",
                    git_failure("Fetch from", url, &error),
                    first_err.class(),
                    first_err.code()
                )
            })?;
    }

    let tracking_ref = format!("refs/remotes/{}/{}", remote_name, remote.branch);
    let target_oid = repo
        .refname_to_id(&tracking_ref)
        .or_else(|_| repo.refname_to_id("FETCH_HEAD"))
        .with_context(|| format!("resolve fetched commit for {}", tracking_ref))?;
    let target_obj = repo.find_object(target_oid, None)?;

    let local_ref = format!("refs/heads/{}", remote.branch);
    if let Ok(mut r) = repo.find_reference(&local_ref) {
        r.set_target(target_oid, "wuddle git sync")?;
    } else {
        let commit = repo.find_commit(target_oid)?;
        let mut branch = repo.branch(&remote.branch, &commit, true)?;
        let upstream = format!("{}/{}", remote_name, remote.branch);
        let _ = branch.set_upstream(Some(&upstream));
    }

    if repo.set_head(&local_ref).is_err() {
        repo.set_head_detached(target_oid)?;
    }
    repo.checkout_tree(&target_obj, Some(CheckoutBuilder::new().force()))?;
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
    Ok(())
}

pub fn sync_repo(url: &str, path: &Path, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    let exists = ensure_git_repo(path)?;
    let effective_url = effective_remote_url(path, url).unwrap_or_else(|| url.to_string());
    let (remote, remote_url) = choose_remote_head_with_url(&effective_url, preferred_branch, None)?;
    if !exists {
        clone_repo(&remote_url, path, &remote.branch)?;
    } else {
        sync_existing_repo(&remote_url, path, &remote)?;
    }

    let local = local_head(path)?.ok_or_else(|| anyhow!("Could not read local git HEAD"))?;
    Ok(GitHeadState {
        oid: local.oid,
        short_oid: local.short_oid,
        branch: remote.branch,
        remote_ref: remote.remote_ref,
    })
}

/// Build an updated standalone worktree without mutating the installed one.
///
/// The remote selected by GAM's upstream rules is used for the clone, then all
/// configured remotes and the active branch's preferred upstream are restored
/// onto the staged clone before it can replace the live worktree.
pub fn sync_repo_to_staging(
    url: &str,
    installed_worktree: Option<&Path>,
    staging_path: &Path,
    preferred_branch: Option<&str>,
) -> Result<GitHeadState> {
    let source_repo = installed_worktree
        .map(Repository::open)
        .transpose()
        .context("open installed addon worktree for staging")?;
    let effective_url = source_repo
        .as_ref()
        .and_then(gam_compat::preferred_remote)
        .map(|remote| remote.url)
        .unwrap_or_else(|| url.to_string());

    let synced = sync_repo(&effective_url, staging_path, preferred_branch)?;
    if let Some(source) = source_repo.as_ref() {
        let staged =
            Repository::open(staging_path).context("open updated staged addon worktree")?;
        restore_remote_configuration(source, &staged, &synced.branch)?;
    }
    Ok(synced)
}

/// GAM follows the checked-out branch's configured upstream. Preserve that
/// choice for existing worktrees and use the database URL only for a new clone
/// or a repository without a configured remote.
pub fn effective_remote_url(path: &Path, fallback: &str) -> Option<String> {
    if let Ok(repo) = Repository::open(path) {
        if let Some(remote) = gam_compat::preferred_remote(&repo) {
            return Some(remote.url);
        }
    }
    let fallback = fallback.trim();
    (!fallback.is_empty()).then(|| fallback.to_string())
}

pub fn remote_head_for_branch(url: &str, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    choose_remote_head_for_branch(url, preferred_branch, None)
}

pub fn remote_head_for_branch_bounded(
    url: &str,
    preferred_branch: Option<&str>,
    timeout: Duration,
    cancelled: Arc<AtomicBool>,
) -> Result<GitHeadState> {
    let control = RemoteOperationControl::bounded(timeout, cancelled);
    choose_remote_head_for_branch(url, preferred_branch, Some(&control))
}

pub fn remote_branches(url: &str) -> Result<Vec<String>> {
    let candidates = git_url_candidates(url);
    if candidates.is_empty() {
        anyhow::bail!("Git URL is empty");
    }

    let mut last_err = None;
    for candidate in candidates {
        match remote_branches_for_url(&candidate, None) {
            Ok(branches) => return Ok(branches),
            Err(e) => {
                last_err = Some((candidate, e));
            }
        }
    }

    if let Some((candidate, e)) = last_err {
        anyhow::bail!(
            "list remote branches {} (last tried {}): {}",
            crate::url_safety::safe_remote_label(url),
            crate::url_safety::safe_remote_label(&candidate),
            e
        );
    }
    anyhow::bail!(
        "list remote branches {}",
        crate::url_safety::safe_remote_label(url)
    );
}

/// Return the target directory for an addon_git clone.
///
/// Clones land directly in `Interface/AddOns/{repo_name}` — the same
/// convention used by GitAddonsManager and the TurtleWoW launcher.  This
/// makes addons installed by any of these tools immediately cross-compatible
/// without requiring a repair step.
///
/// The old staging path (`Interface/AddOns/.wuddle/addon_git/…`) is kept as
/// `addon_repo_legacy_staging_dir` for the one-time migration that moves
/// existing clones to the new location.
pub fn addon_direct_dir(wow_dir: &Path, repo_name: &str) -> PathBuf {
    // Use the raw repo name, matching GAM's behaviour. Forge repo names are
    // already filesystem-safe by the forge's own validation rules.
    wow_dir.join("Interface").join("AddOns").join(repo_name)
}

/// Legacy staging path — used only during the one-time migration.
pub fn addon_repo_legacy_staging_dir(
    wow_dir: &Path,
    host: &str,
    owner: &str,
    repo_name: &str,
) -> PathBuf {
    wow_dir
        .join("Interface")
        .join("AddOns")
        .join(".wuddle")
        .join("addon_git")
        .join(sanitize_fs_component(host))
        .join(sanitize_fs_component(owner))
        .join(sanitize_fs_component(repo_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn bounded_remote_control_observes_cancellation_and_deadlines() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let control =
            RemoteOperationControl::bounded(Duration::from_secs(1), Arc::clone(&cancelled));
        assert!(control.may_continue());
        cancelled.store(true, Ordering::Release);
        assert!(!control.may_continue());

        let expired = RemoteOperationControl {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            cancelled: None,
        };
        assert!(!expired.may_continue());
        assert!(expired.check().is_err());
    }

    fn commit_value(repo: &Repository, root: &Path, value: &str) -> Oid {
        fs::write(root.join("value.txt"), value.as_bytes()).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("Wuddle Test", "test@example.invalid").unwrap();
        let parents = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| repo.find_commit(oid).ok())
            .into_iter()
            .collect::<Vec<_>>();
        let parent_refs = parents.iter().collect::<Vec<_>>();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            value,
            &tree,
            &parent_refs,
        )
        .unwrap()
    }

    #[test]
    fn existing_repo_follows_upstream_without_rewriting_origin() {
        let temp = tempfile::tempdir().unwrap();
        let right_path = temp.path().join("right");
        let wrong_path = temp.path().join("wrong");
        let worktree = temp.path().join("worktree");
        fs::create_dir_all(&right_path).unwrap();
        fs::create_dir_all(&wrong_path).unwrap();
        let right = Repository::init(&right_path).unwrap();
        let wrong = Repository::init(&wrong_path).unwrap();
        commit_value(&right, &right_path, "right-v1");
        commit_value(&wrong, &wrong_path, "wrong-v1");

        sync_repo(&right_path.to_string_lossy(), &worktree, None).unwrap();
        {
            let repo = Repository::open(&worktree).unwrap();
            repo.remote_rename("origin", "gam").unwrap();
            repo.remote("origin", &wrong_path.to_string_lossy())
                .unwrap();
            let head_name = repo.head().unwrap().name().unwrap().to_string();
            let branch_name = head_name.strip_prefix("refs/heads/").unwrap();
            let mut config = repo.config().unwrap();
            config
                .set_str(&format!("branch.{branch_name}.remote"), "gam")
                .unwrap();
            config
                .set_str(
                    &format!("branch.{branch_name}.merge"),
                    &format!("refs/heads/{branch_name}"),
                )
                .unwrap();
        }

        commit_value(&right, &right_path, "right-v2");
        sync_repo(&wrong_path.to_string_lossy(), &worktree, None).unwrap();

        assert_eq!(
            fs::read_to_string(worktree.join("value.txt")).unwrap(),
            "right-v2"
        );
        let repo = Repository::open(&worktree).unwrap();
        assert_eq!(
            repo.find_remote("origin").unwrap().url(),
            Ok(wrong_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            repo.find_remote("gam").unwrap().url(),
            Ok(right_path.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn staged_update_preserves_all_remotes_and_the_configured_upstream() {
        let temp = tempfile::tempdir().unwrap();
        let right_path = temp.path().join("right");
        let wrong_path = temp.path().join("wrong");
        let installed = temp.path().join("installed");
        let staged = temp.path().join("staged");
        fs::create_dir_all(&right_path).unwrap();
        fs::create_dir_all(&wrong_path).unwrap();
        let right = Repository::init(&right_path).unwrap();
        let wrong = Repository::init(&wrong_path).unwrap();
        commit_value(&right, &right_path, "right-v1");
        commit_value(&wrong, &wrong_path, "wrong-v1");

        sync_repo(&right_path.to_string_lossy(), &installed, None).unwrap();
        let branch_name;
        {
            let repo = Repository::open(&installed).unwrap();
            repo.remote_rename("origin", "gam").unwrap();
            repo.remote("origin", &wrong_path.to_string_lossy())
                .unwrap();
            branch_name = repo.head().unwrap().shorthand().unwrap().to_string();
            let mut config = repo.config().unwrap();
            config
                .set_str(&format!("branch.{branch_name}.remote"), "gam")
                .unwrap();
            config
                .set_str(
                    &format!("branch.{branch_name}.merge"),
                    &format!("refs/heads/{branch_name}"),
                )
                .unwrap();
        }
        commit_value(&right, &right_path, "right-v2");

        sync_repo_to_staging(
            &wrong_path.to_string_lossy(),
            Some(&installed),
            &staged,
            None,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(staged.join("value.txt")).unwrap(),
            "right-v2"
        );
        let repo = Repository::open(&staged).unwrap();
        assert_eq!(
            repo.find_remote("origin").unwrap().url(),
            Ok(wrong_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            repo.find_remote("gam").unwrap().url(),
            Ok(right_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            repo.config()
                .unwrap()
                .get_string(&format!("branch.{branch_name}.remote"))
                .unwrap(),
            "gam"
        );
    }

    #[test]
    fn git_errors_do_not_repeat_remote_credentials_or_private_paths() {
        let remote = "https://user:secret@example.org/private/project.git?token=secret";
        let source = git2::Error::from_str(remote);
        let rendered = git_failure("Fetch from", remote, &source).to_string();
        assert!(rendered.contains("example.org"));
        for private in ["user", "secret", "private", "project", "token"] {
            assert!(!rendered.contains(private));
        }
    }
}
