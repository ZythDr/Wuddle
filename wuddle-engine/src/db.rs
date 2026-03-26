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
    /// SHA-256 hex digest recorded at install time (None for pre-migration rows).
    pub sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddonInstallOwner {
    pub repo_id: i64,
    pub owner: String,
    pub name: String,
    pub manifest_path: String,
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
            PRAGMA foreign_keys=ON;
            "#,
        )?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        const SCHEMA_VERSION: i32 = 7;

        let current: i32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;

        if current >= SCHEMA_VERSION {
            return Ok(());
        }

        // v0 → v1: create all tables, apply backward-compatible column additions
        // for DBs that predate this migration system, and run data fixups.
        if current < 1 {
            self.conn.execute_batch(
                r#"
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

            // Add columns missing from DBs created before they were introduced.
            self.ensure_repo_columns()?;

            self.conn
                .execute("UPDATE repos SET enabled=1 WHERE enabled IS NULL", [])?;
            self.conn.execute(
                "UPDATE repos SET git_branch='master' WHERE mode='addon_git' AND (git_branch IS NULL OR TRIM(git_branch)='')",
                [],
            )?;

            self.conn.execute_batch("PRAGMA user_version = 1")?;
        }

        // v1 → v2: add sha256 column to installs for file integrity checking.
        if current < 2 {
            self.conn
                .execute_batch("ALTER TABLE installs ADD COLUMN sha256 TEXT")?;
            self.conn.execute_batch("PRAGMA user_version = 2")?;
        }

        // v2 → v3: add published_at_unix for adaptive update frequency.
        if current < 3 {
            self.conn
                .execute_batch("ALTER TABLE repos ADD COLUMN published_at_unix INTEGER")?;
            self.conn.execute_batch("PRAGMA user_version = 3")?;
        }

        // v3 → v4: normalize host/owner/name to lowercase and deduplicate.
        // The UNIQUE INDEX was case-sensitive, so mixed-case duplicates could slip
        // through when the same repo was added from different URL casings.
        if current < 4 {
            self.migrate_v4_normalize_repos()?;
            self.conn.execute_batch("PRAGMA user_version = 4")?;
        }

        // v4 → v5: repo owner/name need original casing restored.
        // v4 lowercased everything; the actual fix runs in the GUI layer (needs
        // HTTP client) on next startup, then bumps to v6.
        if current < 5 {
            self.conn.execute_batch("PRAGMA user_version = 5")?;
        }

        // v6 → v7: add merge_installs and pinned_version columns.
        if current < 7 {
            let cols = self.existing_repo_columns()?;
            if !cols.contains("merge_installs") {
                self.conn.execute_batch(
                    "ALTER TABLE repos ADD COLUMN merge_installs INTEGER NOT NULL DEFAULT 0",
                )?;
            }
            if !cols.contains("pinned_version") {
                self.conn
                    .execute_batch("ALTER TABLE repos ADD COLUMN pinned_version TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 7")?;
        }

        Ok(())
    }

    /// Returns true if the DB is at schema v5 (needs casing fix from GUI layer).
    pub fn needs_casing_fix(&self) -> bool {
        let ver: i32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap_or(0);
        ver == 5
    }

    /// Mark the casing fix as complete by bumping to v6.
    pub fn mark_casing_fixed(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA user_version = 6")?;
        Ok(())
    }

    /// Update owner and name for a repo by id.
    pub fn update_repo_casing(&self, id: i64, owner: &str, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE repos SET owner=?1, name=?2 WHERE id=?3",
            params![owner, name, id],
        )?;
        Ok(())
    }

    fn migrate_v4_normalize_repos(&self) -> Result<()> {
        // 1. Lowercase all host/owner/name/url values.
        self.conn.execute_batch(
            r#"
            UPDATE repos SET
              host  = LOWER(host),
              owner = LOWER(owner),
              name  = LOWER(name),
              url   = LOWER(url)
            "#,
        )?;

        // 2. Remove duplicates that now collide: keep the row with the highest id
        //    (most recently added) and migrate its installs from older duplicates.
        let dupes: Vec<(String, String, String)> = {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT host, owner, name
                FROM repos
                GROUP BY host, owner, name
                HAVING COUNT(*) > 1
                "#,
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        for (host, owner, name) in &dupes {
            // Find all IDs for this (host, owner, name), ordered descending.
            let ids: Vec<i64> = {
                let mut stmt = self.conn.prepare(
                    "SELECT id FROM repos WHERE host=?1 AND owner=?2 AND name=?3 ORDER BY id DESC",
                )?;
                let rows = stmt.query_map(params![host, owner, name], |row| row.get(0))?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            if ids.len() < 2 {
                continue;
            }
            let keep_id = ids[0];
            for &remove_id in &ids[1..] {
                // Move installs from the duplicate to the keeper (ignore conflicts).
                self.conn.execute(
                    "UPDATE OR IGNORE installs SET repo_id=?1 WHERE repo_id=?2",
                    params![keep_id, remove_id],
                )?;
                // Delete leftover installs that conflicted.
                self.conn.execute(
                    "DELETE FROM installs WHERE repo_id=?1",
                    params![remove_id],
                )?;
                // Delete the duplicate repo.
                self.conn
                    .execute("DELETE FROM repos WHERE id=?1", params![remove_id])?;
            }
        }

        // 3. Recreate the unique index with COLLATE NOCASE for future safety.
        self.conn.execute_batch(
            r#"
            DROP INDEX IF EXISTS idx_repos_unique;
            CREATE UNIQUE INDEX idx_repos_unique
              ON repos(host COLLATE NOCASE, owner COLLATE NOCASE, name COLLATE NOCASE);
            "#,
        )?;

        Ok(())
    }

    fn existing_repo_columns(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(repos)")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(names.into_iter().collect())
    }

    fn ensure_repo_columns(&self) -> Result<()> {
        let names = self.existing_repo_columns()?;

        let ensure = |name: &str, sql: &str| -> Result<()> {
            if !names.contains(name) {
                self.conn.execute(sql, [])?;
            }
            Ok(())
        };

        ensure("git_branch", "ALTER TABLE repos ADD COLUMN git_branch TEXT")?;
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
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url,
              published_at_unix, merge_installs, pinned_version
            )
            VALUES (
              ?1,  ?2,   ?3,   ?4,    ?5,   ?6,   ?7,      ?8,         ?9,         ?10,         ?11,
              ?12,               ?13,                 ?14,                  ?15,
              ?16, ?17, ?18
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
                repo.installed_asset_url,
                repo.published_at_unix,
                if repo.merge_installs { 1 } else { 0 },
                repo.pinned_version,
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
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url,
              published_at_unix, merge_installs, pinned_version
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
                published_at_unix: row.get(16)?,
                merge_installs: row.get::<_, i64>(17).unwrap_or(0) != 0,
                pinned_version: row.get(18)?,
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
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url,
              published_at_unix, merge_installs, pinned_version
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
                published_at_unix: row.get(16)?,
                merge_installs: row.get::<_, i64>(17).unwrap_or(0) != 0,
                pinned_version: row.get(18)?,
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

    pub fn set_published_at(&self, id: i64, published_at_unix: Option<i64>) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET published_at_unix=?1 WHERE id=?2"#,
            params![published_at_unix, id],
        )?;
        Ok(())
    }

    pub fn set_merge_installs(&self, id: i64, merge: bool) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET merge_installs=?1 WHERE id=?2"#,
            params![if merge { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    pub fn set_pinned_version(&self, id: i64, version: Option<&str>) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET pinned_version=?1 WHERE id=?2"#,
            params![version, id],
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
            SELECT path, kind, sha256
            FROM installs
            WHERE repo_id=?1
            ORDER BY kind, path
            "#,
        )?;

        let rows = stmt.query_map(params![repo_id], |row| {
            Ok(InstallEntry {
                path: row.get(0)?,
                kind: row.get(1)?,
                sha256: row.get(2)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn add_install_with_hash(
        &self,
        repo_id: i64,
        path: &str,
        kind: &str,
        sha256: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO installs(repo_id, path, kind, sha256)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![repo_id, path, kind, sha256],
        )?;
        Ok(())
    }

    pub fn set_install_sha256(&self, repo_id: i64, path: &str, sha256: Option<&str>) -> Result<()> {
        self.conn.execute(
            r#"UPDATE installs SET sha256=?1 WHERE repo_id=?2 AND path=?3"#,
            params![sha256, repo_id, path],
        )?;
        Ok(())
    }

    pub fn remove_install(&self, repo_id: i64, path: &str) -> Result<()> {
        self.conn.execute(
            r#"DELETE FROM installs WHERE repo_id=?1 AND path=?2"#,
            params![repo_id, path],
        )?;
        Ok(())
    }

    /// Returns all addon install paths (lowercased) currently tracked across all repos.
    pub fn all_addon_install_paths(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT LOWER(path) FROM installs WHERE kind='addon'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = HashSet::new();
        for r in rows {
            out.insert(r?);
        }
        Ok(out)
    }

    pub fn find_addon_install_owners(
        &self,
        path: &str,
        exclude_repo_id: Option<i64>,
    ) -> Result<Vec<AddonInstallOwner>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT r.id, r.owner, r.name, i.path
            FROM installs i
            JOIN repos r ON r.id = i.repo_id
            WHERE i.kind='addon'
              AND LOWER(i.path)=LOWER(?1)
              AND (?2 IS NULL OR r.id <> ?2)
            ORDER BY r.owner, r.name
            "#,
        )?;

        let rows = stmt.query_map(params![path, exclude_repo_id], |row| {
            Ok(AddonInstallOwner {
                repo_id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                manifest_path: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
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
