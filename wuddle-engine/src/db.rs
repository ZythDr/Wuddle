use anyhow::{Context, Result};
use rusqlite::{params, Connection, Error as SqlError, ErrorCode};
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use crate::model::{InstallMode, Repo};

const SCHEMA_VERSION: i32 = 16;
static DB_OPEN_LOCK: Mutex<()> = Mutex::new(());

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
    /// Release version (tag_name) recorded at install time.
    pub version: Option<String>,
    /// Optional user-facing label. Currently used by tracked MPQ files.
    pub display_name: Option<String>,
    /// Lightweight identity used for managed MPQ status checks.
    pub file_fingerprint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MpqProtectionRow {
    pub path: String,
    /// Lightweight filesystem identity used to notice ordinary replacements
    /// without rereading multi-gigabyte MPQ contents.
    pub fingerprint: String,
    pub protected: bool,
    pub core: bool,
    pub editor_unlocked: bool,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MpqBackupRow {
    pub repo_id: i64,
    pub path: String,
    pub backup_path: String,
    pub sha256: Option<String>,
    pub fingerprint: Option<String>,
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
        // Frontend services use separate short-lived Engine connections. Keep
        // connection initialization and migration serialized within this process
        // as journal-mode setup itself can otherwise race on a brand-new DB.
        let _open_guard = DB_OPEN_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        // Avoid taking a write lock during the normal already-current startup.
        // migrate_locked reads the version again after acquiring the lock.
        let current: i32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if current >= SCHEMA_VERSION {
            return Ok(());
        }

        // Engine operations open short-lived connections independently. Acquire
        // the migration write lock before reading user_version so two operations
        // cannot both decide that the same ALTER TABLE is still required.
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.migrate_locked();
        match result {
            Ok(()) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn migrate_locked(&self) -> Result<()> {
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
                                    installed_asset_url  TEXT,
                                    selected_addons_json TEXT
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

            self.conn.execute_batch("PRAGMA user_version = 1")?;
        }

        // v1 → v2: add sha256 column to installs for file integrity checking.
        if current < 2 {
            let cols = self.existing_install_columns()?;
            if !cols.contains("sha256") {
                self.conn
                    .execute_batch("ALTER TABLE installs ADD COLUMN sha256 TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 2")?;
        }

        // v2 → v3: add published_at_unix for adaptive update frequency.
        if current < 3 {
            let cols = self.existing_repo_columns()?;
            if !cols.contains("published_at_unix") {
                self.conn
                    .execute_batch("ALTER TABLE repos ADD COLUMN published_at_unix INTEGER")?;
            }
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
            // Use ensure_repo_columns style to be safe if columns already exist.
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

        // v7 → v8: add version column to installs for per-file version tracking (WeirdUtils).
        if current < 8 {
            let cols = self.existing_install_columns()?;
            if !cols.contains("version") {
                self.conn
                    .execute_batch("ALTER TABLE installs ADD COLUMN version TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 8")?;
        }

        // v8 -> v9: persist selected addon folders for collection-style addon_git repos.
        if current < 9 {
            let cols = self.existing_repo_columns()?;
            if !cols.contains("selected_addons_json") {
                self.conn
                    .execute_batch("ALTER TABLE repos ADD COLUMN selected_addons_json TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 9")?;
        }

        // v9 -> v10: add installed_at_unix to track when an addon was last installed/updated.
        if current < 10 {
            let cols = self.existing_repo_columns()?;
            if !cols.contains("installed_at_unix") {
                self.conn
                    .execute_batch("ALTER TABLE repos ADD COLUMN installed_at_unix INTEGER")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 10")?;
        }

        // v10 -> v11: MPQ labels, protection state, reversible replacements,
        // and curated bundle dependency ownership.
        if current < 11 {
            let cols = self.existing_install_columns()?;
            if !cols.contains("display_name") {
                self.conn
                    .execute_batch("ALTER TABLE installs ADD COLUMN display_name TEXT")?;
            }
            self.conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS mpq_protection (
                  path       TEXT PRIMARY KEY COLLATE NOCASE,
                  sha256     TEXT NOT NULL,
                  protected  INTEGER NOT NULL DEFAULT 1,
                  core       INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS mpq_backups (
                  repo_id      INTEGER NOT NULL,
                  path         TEXT NOT NULL COLLATE NOCASE,
                  backup_path  TEXT NOT NULL,
                  sha256       TEXT,
                  PRIMARY KEY(repo_id, path),
                  FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS repo_dependencies (
                  parent_repo_id  INTEGER NOT NULL,
                  child_repo_id   INTEGER NOT NULL,
                  relationship    TEXT NOT NULL,
                  PRIMARY KEY(parent_repo_id, child_repo_id, relationship),
                  FOREIGN KEY(parent_repo_id) REFERENCES repos(id) ON DELETE CASCADE,
                  FOREIGN KEY(child_repo_id) REFERENCES repos(id) ON DELETE CASCADE
                );

                PRAGMA user_version = 11;
                "#,
            )?;
        }

        // v11 -> v12: stop hashing every untracked MPQ during protection
        // scans. The legacy sha256 column remains for compatibility with
        // development databases created while MPQ support was in progress.
        if current < 12 {
            let cols = self.existing_mpq_protection_columns()?;
            if !cols.contains("fingerprint") {
                self.conn.execute_batch(
                    "ALTER TABLE mpq_protection \
                     ADD COLUMN fingerprint TEXT NOT NULL DEFAULT ''",
                )?;
            }
            self.conn.execute_batch("PRAGMA user_version = 12")?;
        }

        // v12 -> v13: make routine status and restored-backup checks use the
        // same metadata-only identity as untracked MPQ discovery.
        if current < 13 {
            let install_cols = self.existing_install_columns()?;
            if !install_cols.contains("file_fingerprint") {
                self.conn
                    .execute_batch("ALTER TABLE installs ADD COLUMN file_fingerprint TEXT")?;
            }
            let backup_cols = self.existing_mpq_backup_columns()?;
            if !backup_cols.contains("fingerprint") {
                self.conn
                    .execute_batch("ALTER TABLE mpq_backups ADD COLUMN fingerprint TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 13")?;
        }

        // v13 -> v14: allow untracked/manual MPQs to carry the same kind of
        // persistent friendly label as Wuddle-installed MPQs.
        if current < 14 {
            let cols = self.existing_mpq_protection_columns()?;
            if !cols.contains("display_name") {
                self.conn
                    .execute_batch("ALTER TABLE mpq_protection ADD COLUMN display_name TEXT")?;
            }
            self.conn.execute_batch("PRAGMA user_version = 14")?;
        }

        // v14 -> v15: keep the Manage MPQs editor padlock independent from
        // collision/replacement protection semantics.
        if current < 15 {
            let cols = self.existing_mpq_protection_columns()?;
            if !cols.contains("editor_unlocked") {
                self.conn.execute_batch(
                    "ALTER TABLE mpq_protection \
                     ADD COLUMN editor_unlocked INTEGER NOT NULL DEFAULT 0",
                )?;
            }
            self.conn.execute_batch("PRAGMA user_version = 15")?;
        }

        // v15 -> v16: Wuddle-installed MPQs are editable by default. Rows
        // already present in mpq_protection represent an explicit user choice
        // and are deliberately left unchanged.
        if current < 16 {
            self.conn.execute_batch(
                r#"
                INSERT OR IGNORE INTO mpq_protection(
                  path, sha256, protected, core, fingerprint, display_name, editor_unlocked
                )
                SELECT path, '', 0, 0, COALESCE(file_fingerprint, ''), display_name, 1
                FROM installs
                WHERE kind='mpq';
                PRAGMA user_version = 16;
                "#,
            )?;
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
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
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
                self.conn
                    .execute("DELETE FROM installs WHERE repo_id=?1", params![remove_id])?;
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

    fn existing_install_columns(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(installs)")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(names.into_iter().collect())
    }

    fn existing_mpq_protection_columns(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(mpq_protection)")?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(names.into_iter().collect())
    }

    fn existing_mpq_backup_columns(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(mpq_backups)")?;
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
        ensure(
            "selected_addons_json",
            "ALTER TABLE repos ADD COLUMN selected_addons_json TEXT",
        )?;
        Ok(())
    }

    pub fn add_repo(&self, repo: &Repo) -> Result<i64> {
        let mode_str = repo.mode.as_str();

        let insert_result = self.conn.execute(
            r#"
            INSERT INTO repos(
              url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url, installed_at_unix,
                            published_at_unix, merge_installs, pinned_version, selected_addons_json
            )
            VALUES (
              ?1,  ?2,   ?3,   ?4,    ?5,   ?6,   ?7,      ?8,         ?9,         ?10,         ?11,
              ?12,               ?13,                 ?14,                  ?15,                 ?16,
                            ?17, ?18, ?19, ?20
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
                repo.installed_at_unix,
                repo.published_at_unix,
                if repo.merge_installs { 1 } else { 0 },
                repo.pinned_version,
                repo.selected_addons_json,
            ],
        );

        match insert_result {
            Ok(_) => return Ok(self.conn.last_insert_rowid()),
            Err(SqlError::SqliteFailure(ref err, _))
                if err.code == ErrorCode::ConstraintViolation => {}
            Err(e) => return Err(e.into()),
        }

        let (existing_id, existing_owner, existing_name) = self
            .conn
            .query_row(
                r#"SELECT id, owner, name FROM repos WHERE host=?1 COLLATE NOCASE AND owner=?2 COLLATE NOCASE AND name=?3 COLLATE NOCASE LIMIT 1"#,
                params![repo.host, repo.owner, repo.name],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .or_else(|_| {
                self.conn.query_row(
                    r#"SELECT id, owner, name FROM repos WHERE forge=?1 COLLATE NOCASE AND host=?2 COLLATE NOCASE AND owner=?3 COLLATE NOCASE AND name=?4 COLLATE NOCASE LIMIT 1"#,
                    params![repo.forge, repo.host, repo.owner, repo.name],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
                )
            })?;

        // Best-casing strategy: if the user adds a repo with "better" casing
        // (mixed/upper) than what we have (all-lowercase from v4 migration), update it.
        let mut update_owner = None;
        let mut update_name = None;

        let has_upper = |s: &str| s.chars().any(|c| c.is_uppercase());
        let is_lower = |s: &str| s == s.to_lowercase() && s.chars().any(|c| c.is_alphabetic());

        if has_upper(&repo.owner) && is_lower(&existing_owner) {
            update_owner = Some(&repo.owner);
        }
        if has_upper(&repo.name) && is_lower(&existing_name) {
            update_name = Some(&repo.name);
        }

        if update_owner.is_some() || update_name.is_some() {
            let _ = self.update_repo_casing(
                existing_id,
                update_owner.unwrap_or(&existing_owner),
                update_name.unwrap_or(&existing_name),
            );
        }

        if repo.selected_addons_json.is_some() {
            let _ =
                self.set_repo_selected_addons(existing_id, repo.selected_addons_json.as_deref());
        }

        Ok(existing_id)
    }

    pub fn list_repos(&self) -> Result<Vec<Repo>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id, url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url, installed_at_unix,
                            published_at_unix, merge_installs, pinned_version, selected_addons_json
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
                installed_at_unix: row.get(16)?,
                published_at_unix: row.get(17)?,
                merge_installs: row.get::<_, i64>(18).unwrap_or(0) != 0,
                pinned_version: row.get(19)?,
                selected_addons_json: row.get(20)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn find_repo_by_identity(
        &self,
        host: &str,
        owner: &str,
        name: &str,
    ) -> Result<Option<Repo>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id, url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url, installed_at_unix,
                            published_at_unix, merge_installs, pinned_version, selected_addons_json
            FROM repos
            WHERE host=?1 COLLATE NOCASE AND owner=?2 COLLATE NOCASE AND name=?3 COLLATE NOCASE
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query_map(params![host, owner, name], |row| {
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
                installed_at_unix: row.get(16)?,
                published_at_unix: row.get(17)?,
                merge_installs: row.get::<_, i64>(18).unwrap_or(0) != 0,
                pinned_version: row.get(19)?,
                selected_addons_json: row.get(20)?,
            })
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn get_repo(&self, id: i64) -> Result<Repo> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT
              id, url, forge, host, owner, name, mode, enabled, git_branch, asset_regex, last_version, etag,
              installed_asset_id, installed_asset_name, installed_asset_size, installed_asset_url, installed_at_unix,
                            published_at_unix, merge_installs, pinned_version, selected_addons_json
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
                installed_at_unix: row.get(16)?,
                published_at_unix: row.get(17)?,
                merge_installs: row.get::<_, i64>(18).unwrap_or(0) != 0,
                pinned_version: row.get(19)?,
                selected_addons_json: row.get(20)?,
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

    pub fn set_repo_release_source(
        &self,
        id: i64,
        mode: &InstallMode,
        asset_regex: Option<&str>,
        pinned_version: Option<&str>,
        selected_addons_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE repos
            SET
              mode=?1,
              git_branch=NULL,
              asset_regex=?2,
              pinned_version=?3,
              selected_addons_json=?4,
              etag=NULL,
              enabled=1
            WHERE id=?5
            "#,
            params![
                mode.as_str(),
                asset_regex,
                pinned_version,
                selected_addons_json,
                id
            ],
        )?;
        Ok(())
    }

    pub fn mark_repo_manual(&self, id: i64) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE repos
            SET
              url='',
              forge='manual',
              host='',
              owner='',
              mode='manual',
              git_branch=NULL,
              asset_regex=NULL,
              last_version='Manual',
              etag=NULL,
              installed_asset_id=NULL,
              installed_asset_name=NULL,
              installed_asset_size=NULL,
              installed_asset_url=NULL,
              installed_at_unix=NULL,
              published_at_unix=NULL,
              pinned_version=NULL,
              selected_addons_json=NULL
            WHERE id=?1
            "#,
            params![id],
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
        installed_at_unix: Option<i64>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE repos
            SET
              last_version=?1,
              installed_asset_id=?2,
              installed_asset_name=?3,
              installed_asset_size=?4,
              installed_asset_url=?5,
              installed_at_unix=?6
            WHERE id=?7
            "#,
            params![
                version,
                asset_id,
                asset_name,
                asset_size,
                asset_url,
                installed_at_unix,
                id
            ],
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

    pub fn set_repo_selected_addons(
        &self,
        id: i64,
        selected_addons_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE repos SET selected_addons_json=?1 WHERE id=?2"#,
            params![selected_addons_json, id],
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

    pub fn add_install(
        &self,
        repo_id: i64,
        path: &str,
        kind: &str,
        version: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO installs(repo_id, path, kind, version)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![repo_id, path, kind, version],
        )?;
        Ok(())
    }

    pub fn list_installs(&self, repo_id: i64) -> Result<Vec<InstallEntry>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT path, kind, sha256, version, display_name, file_fingerprint
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
                version: row.get(3)?,
                display_name: row.get(4)?,
                file_fingerprint: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_all_installs_full(&self) -> Result<Vec<(i64, InstallEntry)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT repo_id, path, kind, sha256, version, display_name, file_fingerprint
            FROM installs
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                InstallEntry {
                    path: row.get(1)?,
                    kind: row.get(2)?,
                    sha256: row.get(3)?,
                    version: row.get(4)?,
                    display_name: row.get(5)?,
                    file_fingerprint: row.get(6)?,
                },
            ))
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
        version: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO installs(repo_id, path, kind, sha256, version)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![repo_id, path, kind, sha256, version],
        )?;
        Ok(())
    }

    pub fn add_named_install_with_hash(
        &self,
        repo_id: i64,
        path: &str,
        kind: &str,
        sha256: Option<&str>,
        version: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO installs(repo_id, path, kind, sha256, version, display_name)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![repo_id, path, kind, sha256, version, display_name],
        )?;
        Ok(())
    }

    pub fn commit_mpq_installs(
        &self,
        repo_id: i64,
        installs: &[InstallEntry],
        backups: &[MpqBackupRow],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        // A newly managed MPQ starts unlocked. Reinstalls preserve an
        // existing managed file's explicit editor-lock choice.
        for install in installs {
            tx.execute(
                r#"
                INSERT INTO mpq_protection(
                  path, sha256, protected, core, fingerprint, display_name, editor_unlocked
                ) VALUES (?1, '', 0, 0, COALESCE(?2, ''), ?3, 1)
                ON CONFLICT(path) DO UPDATE SET
                  fingerprint=excluded.fingerprint,
                  display_name=excluded.display_name,
                  protected=0,
                  core=0,
                  editor_unlocked=CASE
                    WHEN EXISTS(
                      SELECT 1 FROM installs
                      WHERE repo_id=?4 AND kind='mpq' AND path=?1 COLLATE NOCASE
                    ) THEN mpq_protection.editor_unlocked
                    ELSE 1
                  END
                "#,
                params![
                    install.path,
                    install.file_fingerprint,
                    install.display_name,
                    repo_id
                ],
            )?;
        }
        tx.execute(
            "DELETE FROM installs WHERE repo_id=?1 AND kind='mpq'",
            params![repo_id],
        )?;
        tx.execute("DELETE FROM mpq_backups WHERE repo_id=?1", params![repo_id])?;
        for install in installs {
            tx.execute(
                r#"
                INSERT OR REPLACE INTO installs(
                  repo_id, path, kind, sha256, version, display_name, file_fingerprint
                )
                VALUES (?1, ?2, 'mpq', ?3, ?4, ?5, ?6)
                "#,
                params![
                    repo_id,
                    install.path,
                    install.sha256,
                    install.version,
                    install.display_name,
                    install.file_fingerprint
                ],
            )?;
        }
        for backup in backups {
            tx.execute(
                r#"
                INSERT OR REPLACE INTO mpq_backups(
                  repo_id, path, backup_path, sha256, fingerprint
                )
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    backup.repo_id,
                    backup.path,
                    backup.backup_path,
                    backup.sha256,
                    backup.fingerprint
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_install_display_name(
        &self,
        repo_id: i64,
        path: &str,
        display_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"UPDATE installs SET display_name=?1 WHERE repo_id=?2 AND path=?3"#,
            params![display_name, repo_id, path],
        )?;
        Ok(())
    }

    pub fn find_mpq_install_owner(&self, path: &str) -> Result<Option<i64>> {
        let result = self.conn.query_row(
            r#"
            SELECT repo_id FROM installs
            WHERE kind='mpq' AND LOWER(path)=LOWER(?1)
            LIMIT 1
            "#,
            params![path],
            |row| row.get(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(SqlError::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn upsert_mpq_protection(
        &self,
        path: &str,
        fingerprint: &str,
        detected_core: bool,
    ) -> Result<MpqProtectionRow> {
        let existing = self.get_mpq_protection(path)?;
        let matching = existing
            .as_ref()
            .filter(|row| row.fingerprint == fingerprint);
        // Preserve an explicit user classification only while this is still
        // the same filesystem object. Replacements return to name detection.
        let core = matching.map(|row| row.core).unwrap_or(detected_core);
        let protected = matching.map(|row| row.protected).unwrap_or(true);
        let editor_unlocked = matching.map(|row| row.editor_unlocked).unwrap_or(false);
        self.conn.execute(
            r#"
            INSERT INTO mpq_protection(path, sha256, protected, core, fingerprint, editor_unlocked)
            VALUES (?1, '', ?2, ?3, ?4, ?5)
            ON CONFLICT(path) DO UPDATE SET
              sha256='',
              protected=excluded.protected,
              core=excluded.core,
              fingerprint=excluded.fingerprint,
              editor_unlocked=excluded.editor_unlocked
            "#,
            params![
                path,
                i64::from(protected),
                i64::from(core),
                fingerprint,
                i64::from(editor_unlocked)
            ],
        )?;
        Ok(MpqProtectionRow {
            path: path.to_string(),
            fingerprint: fingerprint.to_string(),
            protected,
            core,
            editor_unlocked,
            display_name: existing.and_then(|row| row.display_name),
        })
    }

    pub fn get_mpq_protection(&self, path: &str) -> Result<Option<MpqProtectionRow>> {
        let result = self.conn.query_row(
            r#"
            SELECT path, fingerprint, protected, core, editor_unlocked, display_name
            FROM mpq_protection WHERE path=?1 COLLATE NOCASE
            "#,
            params![path],
            |row| {
                Ok(MpqProtectionRow {
                    path: row.get(0)?,
                    fingerprint: row.get(1)?,
                    protected: row.get::<_, i64>(2)? != 0,
                    core: row.get::<_, i64>(3)? != 0,
                    editor_unlocked: row.get::<_, i64>(4)? != 0,
                    display_name: row.get(5)?,
                })
            },
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(SqlError::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn list_mpq_protection(&self) -> Result<Vec<MpqProtectionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, fingerprint, protected, core, editor_unlocked, display_name FROM mpq_protection ORDER BY LOWER(path)",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MpqProtectionRow {
                path: row.get(0)?,
                fingerprint: row.get(1)?,
                protected: row.get::<_, i64>(2)? != 0,
                core: row.get::<_, i64>(3)? != 0,
                editor_unlocked: row.get::<_, i64>(4)? != 0,
                display_name: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn set_mpq_protection(&self, path: &str, fingerprint: &str, protected: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE mpq_protection SET protected=?1
            WHERE path=?2 COLLATE NOCASE AND fingerprint=?3
            "#,
            params![i64::from(protected), path, fingerprint],
        )?;
        Ok(())
    }

    pub fn set_mpq_core_classification(
        &self,
        path: &str,
        fingerprint: &str,
        core: bool,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE mpq_protection
            SET core=?1
            WHERE path=?2 COLLATE NOCASE AND fingerprint=?3
            "#,
            params![i64::from(core), path, fingerprint],
        )?;
        Ok(())
    }

    pub fn set_mpq_editor_unlocked(
        &self,
        path: &str,
        fingerprint: &str,
        editor_unlocked: bool,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE mpq_protection
            SET editor_unlocked=?1
            WHERE path=?2 COLLATE NOCASE AND fingerprint=?3
            "#,
            params![i64::from(editor_unlocked), path, fingerprint],
        )?;
        Ok(())
    }

    pub fn edit_mpq_protection_entry(
        &self,
        old_path: &str,
        new_path: &str,
        fingerprint: &str,
        display_name: &str,
        protected: bool,
        core: bool,
        editor_unlocked: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM mpq_protection WHERE path=?1 COLLATE NOCASE",
            params![old_path],
        )?;
        tx.execute(
            r#"
            INSERT OR REPLACE INTO mpq_protection(
              path, sha256, protected, core, fingerprint, display_name, editor_unlocked
            ) VALUES (?1, '', ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                new_path,
                i64::from(protected),
                i64::from(core),
                fingerprint,
                display_name,
                i64::from(editor_unlocked)
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn edit_tracked_mpq_install(
        &self,
        repo_id: i64,
        old_path: &str,
        new_path: &str,
        display_name: &str,
        path_changed: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            r#"UPDATE installs SET path=?3, display_name=?4
               WHERE repo_id=?1 AND path=?2 COLLATE NOCASE AND kind='mpq'"#,
            params![repo_id, old_path, new_path, display_name],
        )?;
        if path_changed {
            tx.execute(
                "DELETE FROM mpq_backups WHERE repo_id=?1 AND path=?2 COLLATE NOCASE",
                params![repo_id, old_path],
            )?;
        }
        tx.execute(
            "UPDATE mpq_protection SET path=?2 WHERE path=?1 COLLATE NOCASE",
            params![old_path, new_path],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn move_mpq_protection(
        &self,
        old_path: &str,
        new_path: &str,
        fingerprint: &str,
        protected: bool,
        core: bool,
        editor_unlocked: bool,
        display_name: Option<&str>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM mpq_protection WHERE path=?1 COLLATE NOCASE",
            params![old_path],
        )?;
        tx.execute(
            r#"
            INSERT OR REPLACE INTO mpq_protection(
              path, sha256, protected, core, fingerprint, display_name, editor_unlocked
            )
            VALUES (?1, '', ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                new_path,
                i64::from(protected),
                i64::from(core),
                fingerprint,
                display_name,
                i64::from(editor_unlocked),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_mpq_protection_display_name(
        &self,
        path: &str,
        fingerprint: &str,
        display_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE mpq_protection SET display_name=?1
            WHERE path=?2 COLLATE NOCASE AND fingerprint=?3
            "#,
            params![display_name, path, fingerprint],
        )?;
        Ok(())
    }

    pub fn add_mpq_backup(
        &self,
        repo_id: i64,
        path: &str,
        backup_path: &str,
        sha256: Option<&str>,
        fingerprint: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO mpq_backups(
              repo_id, path, backup_path, sha256, fingerprint
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![repo_id, path, backup_path, sha256, fingerprint],
        )?;
        Ok(())
    }

    pub fn get_mpq_backup(&self, repo_id: i64, path: &str) -> Result<Option<MpqBackupRow>> {
        let result = self.conn.query_row(
            r#"
            SELECT repo_id, path, backup_path, sha256, fingerprint
            FROM mpq_backups WHERE repo_id=?1 AND path=?2 COLLATE NOCASE
            "#,
            params![repo_id, path],
            |row| {
                Ok(MpqBackupRow {
                    repo_id: row.get(0)?,
                    path: row.get(1)?,
                    backup_path: row.get(2)?,
                    sha256: row.get(3)?,
                    fingerprint: row.get(4)?,
                })
            },
        );
        match result {
            Ok(row) => Ok(Some(row)),
            Err(SqlError::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn remove_mpq_backup(&self, repo_id: i64, path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM mpq_backups WHERE repo_id=?1 AND path=?2 COLLATE NOCASE",
            params![repo_id, path],
        )?;
        Ok(())
    }

    pub fn remove_mpq_install_and_backup(&self, repo_id: i64, path: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM installs WHERE repo_id=?1 AND path=?2 AND kind='mpq'",
            params![repo_id, path],
        )?;
        tx.execute(
            "DELETE FROM mpq_backups WHERE repo_id=?1 AND path=?2 COLLATE NOCASE",
            params![repo_id, path],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn add_repo_dependency(
        &self,
        parent_repo_id: i64,
        child_repo_id: i64,
        relationship: &str,
    ) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO repo_dependencies(parent_repo_id, child_repo_id, relationship)
            VALUES (?1, ?2, ?3)
            "#,
            params![parent_repo_id, child_repo_id, relationship],
        )?;
        Ok(())
    }

    pub fn list_repo_dependencies(&self, parent_repo_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT child_repo_id, relationship FROM repo_dependencies
            WHERE parent_repo_id=?1 ORDER BY child_repo_id
            "#,
        )?;
        let rows = stmt.query_map(params![parent_repo_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
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

    /// Update an install entry's path in-place (used for staging-area migration).
    pub fn update_install_path(&self, repo_id: i64, old_path: &str, new_path: &str) -> Result<()> {
        self.conn.execute(
            r#"UPDATE installs SET path=?3 WHERE repo_id=?1 AND path=?2"#,
            params![repo_id, old_path, new_path],
        )?;
        Ok(())
    }

    /// Atomically update one or more tracked MPQ paths, their optional backup
    /// ownership keys, and the package-level enabled state.
    pub fn update_mpq_enabled_paths(
        &self,
        repo_id: i64,
        changes: &[(String, String)],
        repo_enabled: bool,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (old_path, new_path) in changes {
            tx.execute(
                r#"UPDATE installs SET path=?3
                   WHERE repo_id=?1 AND path=?2 COLLATE NOCASE AND kind='mpq'"#,
                params![repo_id, old_path, new_path],
            )?;
            tx.execute(
                r#"UPDATE mpq_backups SET path=?3
                   WHERE repo_id=?1 AND path=?2 COLLATE NOCASE"#,
                params![repo_id, old_path, new_path],
            )?;
        }
        tx.execute(
            "UPDATE repos SET enabled=?1 WHERE id=?2",
            params![i64::from(repo_enabled), repo_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Returns all addon install paths (lowercased) currently tracked across all repos.
    pub fn all_addon_install_paths(&self) -> Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT LOWER(path) FROM installs WHERE kind='addon'")?;
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

#[cfg(test)]
mod tests {
    use super::Db;
    use rusqlite::{params, Connection};
    use std::sync::{Arc, Barrier};

    #[test]
    fn repairs_schema_version_behind_existing_mpq_columns() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("partial-migration.sqlite");

        {
            let db = Db::open(&path).unwrap();
            db.conn
                .execute(
                    r#"
                    INSERT INTO repos(url, forge, host, owner, name, mode)
                    VALUES (?1, 'github', 'github.com', 'example', 'mod', 'auto')
                    "#,
                    params!["https://github.com/example/mod"],
                )
                .unwrap();
            let repo_id = db.conn.last_insert_rowid();
            db.conn
                .execute(
                    "INSERT INTO installs(repo_id, path, kind) VALUES (?1, ?2, 'dll')",
                    params![repo_id, "d3d9.dll"],
                )
                .unwrap();

            // Reproduce the affected beta profile: the ALTER TABLE completed,
            // but user_version still says that the migration is pending.
            db.conn.execute_batch("PRAGMA user_version = 11").unwrap();
        }

        let repaired = Db::open(&path).unwrap();
        let version: i32 = repaired
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let repo_count: i64 = repaired
            .conn
            .query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0))
            .unwrap();
        let install_count: i64 = repaired
            .conn
            .query_row("SELECT COUNT(*) FROM installs", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 16);
        assert_eq!(repo_count, 1);
        assert_eq!(install_count, 1);
    }

    #[test]
    fn concurrent_opens_serialize_migrations() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("concurrent-migration.sqlite");
        let barrier = Arc::new(Barrier::new(4));

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    Db::open(&path).map(|_| ())
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        let conn = Connection::open(path).unwrap();
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 16);
    }
}
