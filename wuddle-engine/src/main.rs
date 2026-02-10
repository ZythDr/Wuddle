use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use wuddle_engine::{Engine, InstallMode, InstallOptions};

#[derive(Debug, Parser)]
#[command(name = "wuddle", version, about = "WoW addon/dll updater")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    Add {
        url: String,
        /// auto|addon|dll|mixed|raw
        #[arg(long, default_value = "auto")]
        mode: String,
        /// optional regex override for selecting the release asset
        #[arg(long)]
        asset_regex: Option<String>,
    },
    List,
    Remove {
        id: i64,
        #[arg(long, default_value_t = false)]
        remove_local_files: bool,
        #[arg(long)]
        wow_dir: Option<PathBuf>,
    },
    Check {
        #[arg(long)]
        wow_dir: Option<PathBuf>,
    },
    Update {
        #[arg(long)]
        wow_dir: PathBuf,
        /// Only used for Raw mode (or Auto fallback when asset isn't zip/dll)
        #[arg(long)]
        raw_dest: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        symlink_targets: bool,
        #[arg(long, default_value_t = false)]
        set_xattr_comment: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::open_default()?;

    match cli.cmd {
        Cmd::Add {
            url,
            mode,
            asset_regex,
        } => {
            let mode = InstallMode::from_str(&mode).ok_or_else(|| anyhow::anyhow!("bad mode"))?;
            let id = engine.add_repo(&url, mode, asset_regex)?;
            println!("Added repo id={id}");
        }
        Cmd::List => {
            for r in engine.db().list_repos()? {
                println!(
                    "#{:>3} {:<6} {:<18} {}/{} mode={} url={}",
                    r.id,
                    r.forge,
                    r.host,
                    r.owner,
                    r.name,
                    r.mode.as_str(),
                    r.url
                );
            }
        }
        Cmd::Remove {
            id,
            remove_local_files,
            wow_dir,
        } => {
            let removed = engine.remove_repo(id, wow_dir.as_deref(), remove_local_files)?;
            if remove_local_files {
                println!("Removed repo id={id} and deleted {removed} local path(s).");
            } else {
                println!("Removed repo id={id}");
            }
        }
        Cmd::Check { wow_dir } => {
            let plans = engine.check_updates_with_wow(wow_dir.as_deref()).await?;
            for p in plans {
                if let Some(err) = p.error.as_deref() {
                    println!("{}/{}: error ({})", p.owner, p.name, err);
                } else if p.repair_needed {
                    println!("{}/{}: repair needed ({})", p.owner, p.name, p.latest);
                } else if p.not_modified {
                    println!("{}/{}: (etag) unchanged", p.owner, p.name);
                } else if p.asset_url.is_empty() {
                    println!("{}/{}: up-to-date ({})", p.owner, p.name, p.latest);
                } else {
                    println!(
                        "{}/{}: update {} -> {} (asset {})",
                        p.owner,
                        p.name,
                        p.current.clone().unwrap_or("<none>".into()),
                        p.latest,
                        p.asset_name
                    );
                }
            }
        }
        Cmd::Update {
            wow_dir,
            raw_dest,
            symlink_targets,
            set_xattr_comment,
        } => {
            let raw_dest_ref = raw_dest.as_deref();
            let opts = InstallOptions {
                use_symlinks: symlink_targets,
                set_xattr_comment,
            };
            let plans = engine.apply_updates(&wow_dir, raw_dest_ref, opts).await?;
            let mut updated = 0;
            let mut failed = 0;
            for p in plans {
                if p.applied {
                    updated += 1;
                }
                if p.error.is_some() {
                    failed += 1;
                }
            }
            if failed > 0 {
                println!("Done. Updated {updated} repo(s); {failed} failed.");
            } else {
                println!("Done. Updated {updated} repo(s).");
            }
        }
    }

    Ok(())
}
