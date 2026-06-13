use rusqlite::{params, Connection, Result};
use crate::stack::{ScheduleBlock, Task};

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn new() -> Result<Self> {
        let conn = Connection::open("lockstep.db")?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                default_duration INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS schedule_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER REFERENCES tasks(id),
                day TEXT NOT NULL,
                start_minute INTEGER NOT NULL,
                duration_minutes INTEGER NOT NULL,
                is_done BOOLEAN DEFAULT 0
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    /// Loads schedule blocks for a given day. Seeds a 24h "No plan" block if empty.
    pub fn get_or_create_schedule(&self, day: &str) -> Result<Vec<ScheduleBlock>> {
        let mut stmt = self.conn.prepare(
            "SELECT sb.id, sb.task_id, t.name, sb.day, sb.start_minute, sb.duration_minutes, sb.is_done 
             FROM schedule_blocks sb
             LEFT JOIN tasks t ON sb.task_id = t.id
             WHERE sb.day = ?
             ORDER BY sb.start_minute ASC"
        )?;

        let rows = stmt.query_map([day], |row| {
            let id: i64 = row.get(0)?;
            let task_id: Option<i64> = row.get(1)?;
            let t_name: Option<String> = row.get(2)?;
            let day_str: String = row.get(3)?;
            let start_min: i32 = row.get(4)?;
            let dur_min: i32 = row.get(5)?;
            let is_done: bool = row.get(6)?;

            Ok(ScheduleBlock {
                id: Some(id),
                task_id,
                task_name: t_name.unwrap_or_else(|| "No plan".to_string()),
                day: day_str,
                start_minute: start_min,
                duration_minutes: dur_min,
                is_done,
            })
        })?;

        let mut blocks = Vec::new();
        for r in rows {
            blocks.push(r?);
        }

        if blocks.is_empty() {
            // Seed a single 24-hour "No plan" block
            self.conn.execute(
                "INSERT INTO schedule_blocks (task_id, day, start_minute, duration_minutes, is_done)
                 VALUES (NULL, ?, 0, 1440, 0)",
                [day],
            )?;
            
            // Query again to get the auto-generated id
            return self.get_or_create_schedule(day);
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
            tx.execute(
                "INSERT INTO schedule_blocks (task_id, day, start_minute, duration_minutes, is_done)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    block.task_id,
                    block.day,
                    block.start_minute,
                    block.duration_minutes,
                    block.is_done
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Returns all created tasks.
    pub fn get_all_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare("SELECT id, name, default_duration FROM tasks ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                name: row.get(1)?,
                default_duration: row.get(2)?,
            })
        })?;

        let mut tasks = Vec::new();
        for r in rows {
            tasks.push(r?);
        }
        Ok(tasks)
    }

    /// Creates a new task.
    pub fn add_task(&self, name: &str, default_duration: i32) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tasks (name, default_duration) VALUES (?, ?)",
            params![name, default_duration],
        )?;
        Ok(())
    }
}
