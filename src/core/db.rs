use rusqlite::{params, Connection, Result};
use std::path::PathBuf;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path)?;
        let db = Self { conn };
        db.init_schema()?;

        Ok(db)
    }

    fn get_db_path() -> PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
        path.push("yandex-messenger-native");
        path.push("cache.db");
        path
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                title TEXT,
                chat_type TEXT NOT NULL,
                unread_count INTEGER DEFAULT 0,
                updated_at INTEGER NOT NULL,
                raw_json TEXT NOT NULL
            );
            
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                chat_id TEXT NOT NULL,
                from_id TEXT NOT NULL,
                text TEXT,
                created_at INTEGER NOT NULL,
                raw_json TEXT NOT NULL,
                FOREIGN KEY (chat_id) REFERENCES chats(id)
            );
            
            CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);
            CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
            ",
        )?;
        Ok(())
    }

    pub fn cache_chats(&self, _chats_json: &str) -> Result<()> {
        // Implementation for inserting/updating chats based on JSON
        // In a full implementation, we'd parse the JSON array and upsert rows
        Ok(())
    }

    pub fn get_cached_chats(&self) -> Result<String> {
        // Retrieve ordered chats as a JSON string array
        Ok("[]".to_string())
    }
}
