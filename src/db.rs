#![allow(clippy::let_unit_value)]

use rusqlite::{params, Connection, Result};
use crate::stack::{ScheduleBlock, Task};

pub struct Db {
    conn: Connection,
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Db {
    pub fn new() -> Result<Self> {
        let dot_null_db = "C:\\Users\\Israel\\.nullvector\\lockstep.db";
        let local_db = "lockstep.db";
        
        // Ensure the directory structure exists
        if let Some(parent) = std::path::Path::new(dot_null_db).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Migrate local database on startup if centralized one doesn't exist
        if std::path::Path::new(local_db).exists() && !std::path::Path::new(dot_null_db).exists() {
            let _ = std::fs::copy(local_db, dot_null_db);
            let _ = crate::stack::log_info("Migrated local lockstep.db to centralized path C:\\Users\\Israel\\.nullvector\\lockstep.db");
        }

        let conn = Connection::open(dot_null_db)?;
        
        // Create tasks table with new columns if not exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                default_duration INTEGER NOT NULL,
                notes TEXT DEFAULT '',
                category TEXT DEFAULT 'Unassigned'
            )",
            [],
        )?;

        // Run migrations for tasks columns
        if !has_column(&conn, "tasks", "notes")? {
            conn.execute("ALTER TABLE tasks ADD COLUMN notes TEXT DEFAULT ''", [])?;
        }
        if !has_column(&conn, "tasks", "category")? {
            conn.execute("ALTER TABLE tasks ADD COLUMN category TEXT DEFAULT 'Unassigned'", [])?;
        }

        // Create schedule_blocks table with new task_ids string field
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schedule_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_ids TEXT DEFAULT '',
                day TEXT NOT NULL,
                start_minute INTEGER NOT NULL,
                duration_minutes INTEGER NOT NULL,
                is_done BOOLEAN DEFAULT 0,
                notes TEXT DEFAULT ''
            )",
            [],
        )?;

        // Run migrations for schedule_blocks columns
        if !has_column(&conn, "schedule_blocks", "task_ids")? {
            conn.execute("ALTER TABLE schedule_blocks ADD COLUMN task_ids TEXT DEFAULT ''", [])?;
            // If the old task_id column existed, migrate its values
            if has_column(&conn, "schedule_blocks", "task_id")? {
                conn.execute("UPDATE schedule_blocks SET task_ids = CAST(task_id AS TEXT) WHERE task_id IS NOT NULL", [])?;
            }
        }
        if !has_column(&conn, "schedule_blocks", "notes")? {
            conn.execute("ALTER TABLE schedule_blocks ADD COLUMN notes TEXT DEFAULT ''", [])?;
        }

        let db = Self { conn };
        db.seed_default_tasks()?;
        Ok(db)
    }

    fn seed_default_tasks(&self) -> Result<()> {
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        if count == 0 {
            let defaults = vec![
                ("Sleep", 480, "Sovereign biological recovery", "Health"),
                ("Work", 480, "Focused professional output", "Work"),
                ("Coding", 120, "System building and logic iteration", "Work"),
                ("Exercise", 60, "Kinetic activation", "Health"),
                ("Reading", 30, "Noospheric cross-pollination", "Leisure"),
                ("Meal", 45, "Nutritional refueling", "Health"),
                ("Meditation", 20, "Mushin: Cognitive defragmentation", "Health"),
            ];
            for (name, dur, notes, cat) in defaults {
                self.conn.execute(
                    "INSERT INTO tasks (name, default_duration, notes, category) VALUES (?, ?, ?, ?)",
                    rusqlite::params![name, dur, notes, cat],
                )?;
            }
            let _ = crate::stack::log_info("Seeded default task templates in database.");
        }
        Ok(())
    }

    /// Loads schedule blocks for a given day. Seeds a 24h "No plan" block if empty.
    pub fn get_or_create_schedule(&self, day: &str) -> Result<Vec<ScheduleBlock>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_ids, day, start_minute, duration_minutes, is_done, notes 
             FROM schedule_blocks 
             WHERE day = ?
             ORDER BY start_minute ASC"
        )?;

        let rows = stmt.query_map([day], |row| {
            let id: i64 = row.get(0)?;
            let task_ids_str: String = row.get(1)?;
            let day_str: String = row.get(2)?;
            let start_min: i32 = row.get(3)?;
            let dur_min: i32 = row.get(4)?;
            let is_done: bool = row.get(5)?;
            let notes: String = row.get(6)?;

            // Parse comma-separated task IDs
            let task_ids: Vec<i64> = task_ids_str
                .split(',')
                .filter_map(|s| s.trim().parse::<i64>().ok())
                .collect();

            Ok((id, task_ids, day_str, start_min, dur_min, is_done, notes))
        })?;

        let mut raw_blocks = Vec::new();
        for r in rows {
            raw_blocks.push(r?);
        }

        if raw_blocks.is_empty() {
            // Seed a single 24-hour "No plan" block
            self.conn.execute(
                "INSERT INTO schedule_blocks (task_ids, day, start_minute, duration_minutes, is_done, notes)
                 VALUES ('', ?, 0, 1440, 0, '')",
                [day],
            )?;
            return self.get_or_create_schedule(day);
        }

        let mut blocks = Vec::new();
        for (id, task_ids, day_str, start_min, dur_min, is_done, notes) in raw_blocks {
            // Query task names to build the composite task_name string
            let mut names = Vec::new();
            for &tid in &task_ids {
                let mut name_stmt = self.conn.prepare("SELECT name FROM tasks WHERE id = ?")?;
                if let Ok(name) = name_stmt.query_row([tid], |r| r.get::<_, String>(0)) {
                    names.push(name);
                }
            }

            let task_name = if names.is_empty() {
                "No plan".to_string()
            } else {
                names.join(" + ")
            };

            blocks.push(ScheduleBlock {
                id: Some(id),
                task_ids,
                task_name,
                day: day_str,
                start_minute: start_min,
                duration_minutes: dur_min,
                is_done,
                notes,
            });
        }

        Ok(blocks)
    }

    /// Replaces all schedule blocks for a day inside a transaction.
    pub fn save_schedule_blocks(&mut self, day: &str, blocks: &[ScheduleBlock]) -> Result<()> {
        let tx = self.conn.transaction()?;

        // Delete existing blocks for the day
        tx.execute("DELETE FROM schedule_blocks WHERE day = ?", [day])?;

        // Insert new blocks
        for block in blocks {
            let task_ids_str = block
                .task_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");

            tx.execute(
                "INSERT INTO schedule_blocks (task_ids, day, start_minute, duration_minutes, is_done, notes)
                 VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    task_ids_str,
                    block.day,
                    block.start_minute,
                    block.duration_minutes,
                    block.is_done,
                    block.notes
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Returns all created tasks.
    pub fn get_all_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare("SELECT id, name, default_duration, notes, category FROM tasks ORDER BY category ASC, name ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                name: row.get(1)?,
                default_duration: row.get(2)?,
                notes: row.get(3)?,
                category: row.get(4)?,
            })
        })?;

        let mut tasks = Vec::new();
        for r in rows {
            tasks.push(r?);
        }
        Ok(tasks)
    }

    /// Creates a new task template.
    pub fn add_task(&self, name: &str, default_duration: i32, notes: &str, category: &str) -> Result<()> {
        let cat = if category.trim().is_empty() { "Unassigned" } else { category.trim() };
        self.conn.execute(
            "INSERT INTO tasks (name, default_duration, notes, category) VALUES (?, ?, ?, ?)",
            params![name, default_duration, notes, cat],
        )?;
        let _ = crate::stack::log_info(&format!("Added new task template: {} ({}m, Category: {})", name, default_duration, cat));
        Ok(())
    }

    /// Updates notes for a task template.
    pub fn update_task_notes(&self, task_id: i64, notes: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET notes = ? WHERE id = ?",
            params![notes, task_id],
        )?;
        let _ = crate::stack::log_info(&format!("Updated notes for task template ID {}", task_id));
        Ok(())
    }

    /// Updates category for a task template.
    pub fn update_task_category(&self, task_id: i64, category: &str) -> Result<()> {
        let cat = if category.trim().is_empty() { "Unassigned" } else { category.trim() };
        self.conn.execute(
            "UPDATE tasks SET category = ? WHERE id = ?",
            params![cat, task_id],
        )?;
        let _ = crate::stack::log_info(&format!("Updated category for task template ID {} to {}", task_id, cat));
        Ok(())
    }
}
