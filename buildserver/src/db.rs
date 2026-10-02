use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
    sync::Mutex,
    time::Duration,
};

use rusqlite::{Connection, params, types::Value};

pub struct Database(Mutex<Connection>);

impl Database {
    pub fn get(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        rusqlite::vtab::array::load_module(&conn)?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS project (
                name TEXT PRIMARY KEY,
                rootfs_hash TEXT NOT NULL
            ) STRICT;

            CREATE TABLE IF NOT EXISTS job (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                target_arch TEXT NOT NULL,
                status INTEGER NOT NULL
            ) STRICT;

            CREATE TABLE IF NOT EXISTS job_options (
                job_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY(job_id, key),
                FOREIGN KEY(job_id) REFERENCES job(id) ON DELETE CASCADE
            ) STRICT;

            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL,
                status INTEGER NOT NULL,

                FOREIGN KEY (job_id) REFERENCES job(id)
                FOREIGN KEY(job_id) REFERENCES job(id) ON DELETE CASCADE
            ) STRICT;

            CREATE INDEX IF NOT EXISTS idx_tasks_job_id ON tasks(job_id)
            ",
        )?;

        Ok(Self(Mutex::new(conn)))
    }

    pub fn update_project_rootfs(&self, project: &str, rootfs_hash: &str) -> Result<(), rusqlite::Error> {
        self.0
            .lock()
            .unwrap()
            .execute("REPLACE INTO project (name, rootfs_hash) VALUES (?, ?)", params![project, rootfs_hash])?;
        Ok(())
    }

    pub fn get_all_rootfs_hashes(&self) -> Result<HashSet<String>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT rootfs_hash FROM project")?;
        stmt.query_map([], |row| row.get::<usize, String>(0))?.collect::<Result<HashSet<_>, _>>()
    }

    pub fn filter_projects(&self, preserve: impl Iterator<Item = impl AsRef<str>>) -> Result<(), rusqlite::Error> {
        self.0.lock().unwrap().execute(
            "DELETE FROM project WHERE name NOT IN rarray(?)",
            params![Rc::new(preserve.map(|s| Value::from(s.as_ref().to_string())).collect::<Vec<_>>())],
        )?;
        Ok(())
    }

    pub fn create_job(&self, project: &str, target_arch: &str, options: HashMap<&str, &str>) -> Result<i64, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let tx = conn.unchecked_transaction()?;

        let id = tx.query_one(
            "INSERT INTO job (project, target_arch, status) VALUES (?, ?, ?) RETURNING id",
            params![project, target_arch, 0 as i64],
            |row| row.get::<usize, i64>(0),
        )?;

        for (k, v) in options {
            tx.execute("INSERT INTO job_options (job_id, key, value) VALUES (?, ?, ?)", params![id as i64, k, v])?;
        }

        tx.commit()?;

        Ok(id)
    }

    pub fn get_project_jobs(&self, project: &str) -> Result<Vec<u64>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM job WHERE project = ?")?;
        stmt.query_map(params![project], |row| Ok(row.get::<usize, i64>(0)? as u64))?
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn get_job_project(&self, id: i64) -> Result<Option<String>, rusqlite::Error> {
        match self
            .0
            .lock()
            .unwrap()
            .query_one("SELECT project FROM job WHERE id = ?", params![id], |row| row.get::<usize, String>(0))
        {
            Ok(project) => Ok(Some(project)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
