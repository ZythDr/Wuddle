use anyhow::{anyhow, Context, Result};
use git2::{
    build::{CheckoutBuilder, RepoBuilder},
    Cred, Direction, FetchOptions, Oid, RemoteCallbacks, Repository,
};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

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

fn remote_callbacks() -> RemoteCallbacks<'static> {
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
    cb
}

fn git_url_candidates(url: &str) -> Vec<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let base = trimmed.trim_end_matches('/').to_string();
    let mut out = Vec::new();
    let add_dot_git = (base.starts_with("https://")
        || base.starts_with("http://")
        || base.starts_with("git@"))
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

fn remote_refs_for_url(url: &str) -> Result<Vec<RemoteRefInfo>> {
    let tmp = tempdir().context("create temporary git dir")?;
    let bare_repo = Repository::init_bare(tmp.path()).context("init temporary bare repo")?;
    let mut remote = bare_repo
        .remote_anonymous(url)
        .context("create anonymous remote")?;

    // Try credential-aware connect first (works for both public and private remotes),
    // then fall back to plain anonymous fetch if needed.
    let auth_res = remote
        .connect_auth(Direction::Fetch, Some(remote_callbacks()), None)
        .map(|_| ());
    if let Err(auth_err) = auth_res {
        remote
            .connect(Direction::Fetch)
            .with_context(|| format!("connect remote {} (auth failed: {})", url, auth_err))?;
    }
    let refs = remote
        .list()
        .context("list remote refs")?
        .iter()
        .map(|h| RemoteRefInfo {
            name: h.name().to_string(),
            symref_target: h.symref_target().map(|s| s.to_string()),
            oid: h.oid(),
        })
        .collect::<Vec<_>>();

    remote.disconnect()?;
    Ok(refs)
}

fn choose_remote_head_for_url(url: &str, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    let refs = remote_refs_for_url(url)?;

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
) -> Result<(GitHeadState, String)> {
    let candidates = git_url_candidates(url);
    if candidates.is_empty() {
        anyhow::bail!("Git URL is empty");
    }

    let mut last_err = None;
    for candidate in candidates {
        match choose_remote_head_for_url(&candidate, preferred_branch) {
            Ok(state) => return Ok((state, candidate)),
            Err(e) => {
                last_err = Some((candidate, e));
            }
        }
    }

    if let Some((candidate, e)) = last_err {
        anyhow::bail!(
            "connect remote {} (last tried {}): {}",
            url,
            candidate,
            e
        );
    }
    anyhow::bail!("connect remote {}", url);
}

fn choose_remote_head_for_branch(url: &str, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    choose_remote_head_with_url(url, preferred_branch).map(|(state, _)| state)
}

fn remote_branches_for_url(url: &str) -> Result<Vec<String>> {
    let refs = remote_refs_for_url(url)?;
    let mut branches = refs
        .into_iter()
        .filter_map(|r| {
            r.name
                .strip_prefix("refs/heads/")
                .map(|s| s.to_string())
        })
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
    fo.remote_callbacks(remote_callbacks());
    let mut builder = RepoBuilder::new();
    builder.fetch_options(fo);
    if !branch.trim().is_empty() {
        builder.branch(branch);
    }
    builder.clone(url, path).with_context(|| {
        format!(
            "clone {} -> {} (plain failed: {})",
            url,
            path.display(),
            first_err
        )
    })?;
    Ok(())
}

fn sync_existing_repo(url: &str, path: &Path, remote: &GitHeadState) -> Result<()> {
    let repo = Repository::open(path).with_context(|| format!("open repo {}", path.display()))?;
    let mut origin = match repo.find_remote("origin") {
        Ok(_) => {
            repo.remote_set_url("origin", url)
                .with_context(|| format!("set origin remote {}", url))?;
            repo.find_remote("origin")
                .context("re-open origin remote after URL update")?
        }
        Err(_) => repo
            .remote("origin", url)
            .with_context(|| format!("add origin remote {}", url))?,
    };

    let plain_fetch = origin
        .fetch(&[remote.remote_ref.as_str()], None, None)
        .or_else(|_| origin.fetch(&[remote.branch.as_str()], None, None));
    if let Err(first_err) = plain_fetch {
        let mut fo = FetchOptions::new();
        fo.remote_callbacks(remote_callbacks());
        origin
            .fetch(&[remote.remote_ref.as_str()], Some(&mut fo), None)
            .or_else(|_| origin.fetch(&[remote.branch.as_str()], Some(&mut fo), None))
            .with_context(|| {
                format!(
                    "fetch {} {} (plain failed: {})",
                    remote.remote_ref, url, first_err
                )
            })?;
    }

    let tracking_ref = format!("refs/remotes/origin/{}", remote.branch);
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
        repo.branch(&remote.branch, &commit, true)?;
    }

    if repo.set_head(&local_ref).is_err() {
        repo.set_head_detached(target_oid)?;
    }
    repo.checkout_tree(&target_obj, Some(CheckoutBuilder::new().force()))?;
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;
    Ok(())
}

pub fn sync_repo(url: &str, path: &Path, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    let (remote, remote_url) = choose_remote_head_with_url(url, preferred_branch)?;
    let exists = ensure_git_repo(path)?;
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

pub fn remote_head_for_branch(url: &str, preferred_branch: Option<&str>) -> Result<GitHeadState> {
    choose_remote_head_for_branch(url, preferred_branch)
}

pub fn remote_branches(url: &str) -> Result<Vec<String>> {
    let candidates = git_url_candidates(url);
    if candidates.is_empty() {
        anyhow::bail!("Git URL is empty");
    }

    let mut last_err = None;
    for candidate in candidates {
        match remote_branches_for_url(&candidate) {
            Ok(branches) => return Ok(branches),
            Err(e) => {
                last_err = Some((candidate, e));
            }
        }
    }

    if let Some((candidate, e)) = last_err {
        anyhow::bail!(
            "list remote branches {} (last tried {}): {}",
            url,
            candidate,
            e
        );
    }
    anyhow::bail!("list remote branches {}", url);
}

pub fn addon_repo_staging_dir(wow_dir: &Path, host: &str, owner: &str, repo_name: &str) -> PathBuf {
    wow_dir
        .join("Interface")
        .join("AddOns")
        .join(".wuddle")
        .join("addon_git")
        .join(sanitize_fs_component(host))
        .join(sanitize_fs_component(owner))
        .join(sanitize_fs_component(repo_name))
}
