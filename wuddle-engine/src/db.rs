use anyhow::{Context, Result};
use rusqlite::{params, Connection, Error as SqlError, ErrorCode};
use std::collections::HashSet;
use std::time::Duration;

use crate::model::{InstallMode, Repo};

#[derive(Debug, Clone)]
pub struct InstallEntry {
    /// Path relative to WoW root (preferred), e.g:
    /// - "Interact.dll"
    /// - "Interface/AddOns/Interact"
    pub path: String,
    /// "dll" | "addon" | "raw"
    pub kind: String,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path).context("open sqlite db")?;
        conn.busy_timeout(Duration::from_millis(8000))?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            "#,
        )?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        // repos: tracked projects
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS repos (
              id            INTEGER PRIMARY KEY AUTOINCREMENT,
              url           TEXT NOT NULL,
              forge         TEXT NOT NULL,
              host          TEXT NOT NULL,
              owner         TEXT NOT NULL,
              name          TEXT NOT NULL,
              mode          TEXT NOT NULL,
              enabled       INTEGER NOT NULL DEFAULT 1,
              git_branch    TEXT,
              asset_regex   TEXT,
              last_version  TEXT,
              etag          TEXT,
              installed_asset_id   TEXT,
              installed_asset_name TEXT,
              installed_asset_size INTEGER,
              installed_asset_url  TEXT
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_repos_unique
              ON repos(host, owner, name);

            -- installs: what we installed last time for a repo
            CREATE TABLE IF NOT EXISTS installs (
              repo_id INTEGER NOT NULL,
              path    TEXT NOT NULL,
              kind    TEXT NOT NULL,
              PRIMARY KEY(repo_id, path),
              FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_installs_repo
              ON installs(repo_id);

            CREATE TABLE IF NOT EXISTS rate_limits (
              host        TEXT PRIMARY KEY,
              reset_epoch INTEGER NOT NULL
            );
            "#,
        )?;

        // Backward-compatible schema upgrades for existing DBs.
        self.ensure_repo_columns()?;
        self.conn
            .execute("UPDATE repos SET enabled=1 WHERE enabled IS NULL", [])?;
        self.conn.execute(
            "UPDATE repos SET git_branch='master' WHERE mode='addon_git' AND (git_branch IS NULL OR TRIM(git_branch)='')",
            [],
        )?;

        Ok(())
    }

    fn ensure_repo_columns(&self) -> Result<()> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(repos)")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let names: HashSet<String> = names.into_iter().collect();

        let ensure = |name: &str, sql: &str| -> Result<()> {
            if !names.contains(name) {
                self.conn.execute(sql, [])?;
            }
            Ok(())
        };

        ensure(
            "git_branch",
            "ALTER TABLE repos ADD COLUMN git_branch TEXT",
        )?;
        ensure(
            "enabled",
            "ALTER TABLE repos ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1",
        )?;
        ensure(
            "installed_asset_id",
            "ALTER TABLE repos ADD COLUMN installed_asset_id TEXT",
        )?;
        ensure(
            "installed_asset_name",
            "ALTER TABLE repos ADD COLUMN installed_asset_name TEXT",
        )?;
        ensure(
            "installed_asset_size",
            "ALTER TABLE repos ADD COLUMN installed_asset_size INTEGER",
        )?;
        ensure(
            "installed_asset_url",
            "ALTER TABLE repos ADD COLUMN installed_asset_url TEXT",
        )?;
        Ok(())
    }

    pub fn add_repo(&self, repo: &Repo) -> Result<i64> {
        let mode_str = repo.mode.as_str();

        let insert_result = self.conn.execute(
            r#"
            INSERT INTO repos(
              url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url
            )
            VALUES (
              ?1,  ?2,   ?3,   ?4,    ?5,   ?6,   ?7,      ?8,         ?9,         ?10,         ?11,
              ?12,               ?13,                 ?14,                  ?15
            )
            "#,
            params![
                repo.url,
                repo.forge,
                repo.host,
                repo.owner,
                repo.name,
                mode_str,
                if repo.enabled { 1 } else { 0 },
                repo.git_branch,
                repo.asset_regex,
                repo.last_version,
                repo.etag,
                repo.installed_asset_id,
                repo.installed_asset_name,
                repo.installed_asset_size,
                repo.installed_asset_url
            ],
        );

        match insert_result {
            Ok(_) => return Ok(self.conn.last_insert_rowid()),
            Err(SqlError::SqliteFailure(ref err, _))
                if err.code == ErrorCode::ConstraintViolation => {}
            Err(e) => return Err(e.into()),
        }

        let existing_id = self
            .conn
            .query_row(
                r#"SELECT id FROM repos WHERE host=?1 AND owner=?2 AND name=?3 LIMIT 1"#,
                params![repo.host, repo.owner, repo.name],
                |row| row.get::<_, i64>(0),
            )
            .or_else(|_| {
                self.conn.query_row(
                    r#"SELECT id FROM repos WHERE forge=?1 AND host=?2 AND owner=?3 AND name=?4 LIMIT 1"#,
                    params![repo.forge, repo.host, repo.owner, repo.name],
                    |row| row.get::<_, i64>(0),
                )
            })?;
        Ok(existing_id)
    }

    pub fn list_repos(&self) -> Result<Vec<Repo>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id, url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url
            FROM repos
            ORDER BY host, owner, name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            let mode_str: String = row.get(6)?;
            Ok(Repo {
                id: row.get(0)?,
                url: row.get(1)?,
                forge: row.get(2)?,
                host: row.get(3)?,
                owner: row.get(4)?,
                name: row.get(5)?,
                enabled: row.get::<_, i64>(7)? != 0,
                mode: InstallMode::from_str(&mode_str).unwrap_or(InstallMode::Auto),
                git_branch: row.get(8)?,
                asset_regex: row.get(9)?,
                last_version: row.get(10)?,
                etag: row.get(11)?,
                installed_asset_id: row.get(12)?,
                installed_asset_name: row.get(13)?,
                installed_asset_size: row.get(14)?,
                installed_asset_url: row.get(15)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_repo(&self, id: i64) -> Result<Repo> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id, url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url
            FROM repos
            WHERE id=?1
            "#,
        )?;

        let repo = stmt.query_row(params![id], |row| {
            let mode_str: String = row.get(6)?;
            Ok(Repo {
                id: row.get(0)?,
                url: row.get(1)?,
                forge: row.get(2)?,
                host: row.get(3)?,
                owner: row.get(4)?,
                name: row.get(5)?,
                enabled: row.get::<_, i64>(7)? != 0,
                mode: InstallMode::from_str(&mode_str).unwrap_or(InstallMode::Auto),
                git_branch: row.get(8)?,
                asset_regex: row.get(9)?,
                last_version: row.get(10)?,
                etag: row.get(11)?,
                installed_asset_id: row.get(12)?,
                installed_asset_name: row.get(13)?,
                installed_asset_size: row.get(14)?,
                installed_asset_url: row.get(15)?,
            })
        })?;

        Ok(repo)
    }

    pub fn set_last_version(&self, id: i64, version: Option<&str>) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET last_version=?1 WHERE id=?2"#,
            params![version, id],
        )?;
        Ok(())
    }

    pub fn update_etag(&self, id: i64, etag: Option<&str>) -> Result<()> {
        self.conn
            .execute(r#"UPDATE repos SET etag=?1 WHERE id=?2"#, params![etag, id])?;
        Ok(())
    }

    pub fn set_repo_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET enabled=?1 WHERE id=?2"#,
            params![if enabled { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    pub fn set_repo_git_branch(&self, id: i64, git_branch: Option<&str>) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET git_branch=?1 WHERE id=?2"#,
            params![git_branch, id],
        )?;
        Ok(())
    }

    pub fn set_installed_asset_state(
        &self,
        id: i64,
        version: Option<&str>,
        asset_id: Option<&str>,
        asset_name: Option<&str>,
        asset_size: Option<i64>,
        asset_url: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE repos
            SET
              last_version=?1,
              installed_asset_id=?2,
              installed_asset_name=?3,
              installed_asset_size=?4,
              installed_asset_url=?5
            WHERE id=?6
            "#,
            params![version, asset_id, asset_name, asset_size, asset_url, id],
        )?;
        Ok(())
    }

    pub fn remove_repo(&self, id: i64) -> Result<()> {
        // installs rows will be deleted via ON DELETE CASCADE
        self.conn
            .execute(r#"DELETE FROM repos WHERE id=?1"#, params![id])?;
        Ok(())
    }

    // ---------------------------
    // Installs manifest (per repo)
    // ---------------------------

    pub fn clear_installs(&self, repo_id: i64) -> Result<()> {
        self.conn
            .execute(r#"DELETE FROM installs WHERE repo_id=?1"#, params![repo_id])?;
        Ok(())
    }

    pub fn add_install(&self, repo_id: i64, path: &str, kind: &str) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO installs(repo_id, path, kind)
            VALUES (?1, ?2, ?3)
            "#,
            params![repo_id, path, kind],
        )?;
        Ok(())
    }

    pub fn list_installs(&self, repo_id: i64) -> Result<Vec<InstallEntry>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT path, kind
            FROM installs
            WHERE repo_id=?1
            ORDER BY kind, path
            "#,
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(InstallEntry {
                path: row.get(0)?,
                kind: row.get(1)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn set_rate_limit(&self, host: &str, reset_epoch: i64) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO rate_limits(host, reset_epoch)
            VALUES (?1, ?2)
            ON CONFLICT(host) DO UPDATE SET reset_epoch=excluded.reset_epoch
            "#,
            params![host, reset_epoch],
        )?;
        Ok(())
    }

    pub fn get_rate_limit(&self, host: &str) -> Result<Option<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT reset_epoch FROM rate_limits WHERE host=?1")?;
        let mut rows = stmt.query(params![host])?;
        if let Some(row) = rows.next()? {
            let v: i64 = row.get(0)?;
            return Ok(Some(v));
        }
        Ok(None)
    }

    pub fn clear_rate_limit(&self, host: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM rate_limits WHERE host=?1", params![host])?;
        Ok(())
    }
}
