use std::{
    collections::{HashMap, HashSet},
    path::Path,
    rc::Rc,
    sync::Mutex,
    time::Duration,
};

use rusqlite::{Connection, params, types::Value};

use crate::job::{JobTask, JobTaskKind, JobTaskStatus};

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
                name TEXT NOT NULL,
                kind INTEGER NOT NULL,
                status INTEGER NOT NULL,
                input_hash BLOB NOT NULL,

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
            params![project, target_arch, JobTaskStatus::Pending as i64],
            |row| row.get::<usize, i64>(0),
        )?;

        for (k, v) in options {
            tx.execute("INSERT INTO job_options (job_id, key, value) VALUES (?, ?, ?)", params![id as i64, k, v])?;
        }

        tx.commit()?;

        Ok(id)
    }

    pub fn create_task(&self, job_id: i64, name: &str, kind: JobTaskKind, status: JobTaskStatus, input_hash: u128) -> Result<i64, rusqlite::Error> {
        let conn = self.0.lock().unwrap();

        let id = conn.query_one(
            "INSERT INTO tasks (job_id, name, kind, status, input_hash) VALUES (?, ?, ?, ?, ?) RETURNING id",
            params![job_id, name, kind as i64, status as i64, input_hash as i128],
            |row| row.get::<usize, i64>(0),
        )?;

        Ok(id)
    }

    pub fn update_task_status(&self, task_id: i64, status: JobTaskStatus) -> Result<(), rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE tasks SET status = ? WHERE id = ?", params![status as i64, task_id])?;
        Ok(())
    }

    pub fn get_job_tasks(&self, job_id: i64) -> Result<Vec<JobTask>, rusqlite::Error> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, status, input_hash
                 FROM tasks
                 WHERE job_id = ?",
        )?;

        stmt.query_map([job_id], |row| {
            Ok(JobTask {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: (row.get::<usize, i64>(2)? as i64).try_into().expect("Invalid JobTask kind value"),
                status: (row.get::<usize, i64>(3)? as i64).try_into().expect("Invalid JobTask status value"),
                input_hash: (row.get::<usize, i128>(4)? as u128),
            })
        })?
        .collect::<Result<Vec<_>, _>>()
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
