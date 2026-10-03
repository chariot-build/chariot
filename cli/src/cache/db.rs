use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    sync::Mutex,
    time::Duration,
};

use rusqlite::{Connection, Transaction, params};

pub struct Database(Mutex<Connection>);

impl Database {
    pub fn get(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS profile (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                arch TEXT NOT NULL
            ) STRICT;

            CREATE TABLE IF NOT EXISTS profile_option (
                profile_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY(profile_id, key),
                FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
            ) STRICT;

            CREATE TABLE IF NOT EXISTS cached_hash (
                profile_id INTEGER NOT NULL,
                category TEXT NOT NULL,
                hash BLOB NOT NULL,
                PRIMARY KEY(profile_id, category, hash),
                FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
            ) STRICT;
            ",
        )?;

        Ok(Self(Mutex::new(conn)))
    }

    fn get_profile_id(tx: &Transaction, arch: &str, options: &HashMap<&String, &String>) -> Result<Option<i64>, rusqlite::Error> {
        let candidates: Vec<i64> = tx
            .prepare_cached("SELECT id FROM profile WHERE arch = ?")?
            .query_map([arch], |r| r.get(0))?
            .collect::<Result<_, _>>()?;

        let mut opt_stmt = tx.prepare_cached("SELECT key, value FROM profile_option WHERE profile_id = ?")?;
        for id in candidates {
            let existing: BTreeMap<String, String> = opt_stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
            if &existing.iter().collect::<HashMap<_, _>>() == options {
                return Ok(Some(id));
            }
        }

        Ok(None)
    }

    fn create_or_get_profile_id(tx: &Transaction, arch: &str, options: &HashMap<&String, &String>) -> Result<i64, rusqlite::Error> {
        if let Some(id) = Self::get_profile_id(tx, arch, options)? {
            return Ok(id);
        }

        tx.execute("INSERT INTO profile (arch) VALUES (?)", [arch])?;
        let id = tx.last_insert_rowid();

        let mut ins = tx.prepare_cached("INSERT INTO profile_option (profile_id, key, value) VALUES (?, ?, ?)")?;
        for (k, v) in options {
            ins.execute(params![id, k, v])?;
        }

        Ok(id)
    }

    pub fn profile_cache_hashes(
        &self,
        create_profile: bool,
        arch: &str,
        options: &HashMap<&String, &String>,
        hashes: &HashSet<(String, u128)>,
    ) -> Result<bool, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let profile_id = if create_profile {
            Self::create_or_get_profile_id(&tx, arch, options)?
        } else {
            match Self::get_profile_id(&tx, arch, options)? {
                Some(id) => id,
                None => return Ok(false),
            }
        };

        let mut stmt = tx.prepare_cached("INSERT OR IGNORE INTO cached_hash (profile_id, category, hash) VALUES (?, ?, ?)")?;
        for hash in hashes {
            stmt.execute(params![profile_id, hash.0, hash.1.to_be_bytes()])?;
        }

        Ok(true)
    }

    pub fn all_cached_hashes(&self) -> Result<HashSet<(String, u128)>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT category, hash FROM cached_hash")?;
        stmt.query_map([], |row| {
            Ok((row.get::<usize, String>(0)?, u128::from_be_bytes(row.get::<usize, [u8; 16]>(1)?)))
        })?
        .collect::<Result<HashSet<_>, _>>()
    }

    pub fn all_cached_profiles(&self) -> Result<Vec<(String, HashMap<String, String>)>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let mut stmt = tx.prepare("SELECT id, arch FROM profile")?;
        let profile_arches = stmt
            .query_map([], |row| Ok((row.get::<usize, i64>(0)?, row.get::<usize, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut profiles = Vec::new();
        let mut stmt = tx.prepare_cached("SELECT key, value FROM profile_option WHERE profile_id = ?")?;
        for (profile_id, arch) in profile_arches {
            let options = stmt
                .query_map(params![profile_id], |row| {
                    Ok((row.get::<usize, String>(0)?, row.get::<usize, String>(1)?))
                })?
                .collect::<Result<HashMap<_, _>, _>>()?;
            profiles.push((arch, options));
        }

        Ok(profiles)
    }
}
