use rusqlite::{Connection, Result};

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
}
