//! SQLite L2 cache for chats and messages.

use crate::models::{Chat, Message};
use rusqlite::{params, Connection, OptionalExtension, Result as SqlResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open() -> SqlResult<Self> {
        let db_path = Self::get_db_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(db_path)?;
        Self::from_connection(conn)
    }

    pub fn from_connection(conn: Connection) -> SqlResult<Self> {
        let _ = conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            ",
        );
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn get_db_path() -> PathBuf {
        let mut path = dirs::data_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
        path.push("yandex-messenger-native");
        path.push("cache.db");
        path
    }

    fn init_schema(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS chats (
                id TEXT PRIMARY KEY,
                title TEXT,
                chat_type TEXT NOT NULL,
                unread_count INTEGER DEFAULT 0,
                pinned INTEGER DEFAULT 0,
                muted INTEGER DEFAULT 0,
                archived INTEGER DEFAULT 0,
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
            CREATE INDEX IF NOT EXISTS idx_chats_updated ON chats(updated_at DESC);
            ",
        )?;
        Ok(())
    }

    /// Upsert full chat list (replaces metadata; keeps messages).
    pub fn upsert_chats(&self, chats: &[Chat]) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "
                INSERT INTO chats (id, title, chat_type, unread_count, pinned, muted, archived, updated_at, raw_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(id) DO UPDATE SET
                    title=excluded.title,
                    chat_type=excluded.chat_type,
                    unread_count=excluded.unread_count,
                    pinned=excluded.pinned,
                    muted=excluded.muted,
                    archived=excluded.archived,
                    updated_at=excluded.updated_at,
                    raw_json=excluded.raw_json
                ",
            )?;
            for chat in chats {
                let raw = serde_json::to_string(chat).unwrap_or_else(|_| "{}".into());
                let chat_type = format!("{:?}", chat.chat_type).to_lowercase();
                let updated = chat
                    .updated
                    .or(chat.created)
                    .map(|t| t.timestamp())
                    .unwrap_or(0);
                stmt.execute(params![
                    chat.id,
                    chat.title,
                    chat_type,
                    chat.unread_count as i64,
                    chat.pinned as i64,
                    chat.muted as i64,
                    chat.archived as i64,
                    updated,
                    raw,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_chats(&self) -> SqlResult<Vec<Chat>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT raw_json FROM chats ORDER BY pinned DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let raw: String = row.get(0)?;
            Ok(raw)
        })?;
        let mut out = Vec::new();
        for r in rows {
            let raw = r?;
            if let Ok(chat) = serde_json::from_str::<Chat>(&raw) {
                out.push(chat);
            }
        }
        Ok(out)
    }

    /// Upsert messages for a chat (merge by id).
    pub fn upsert_messages(&self, chat_id: &str, messages: &[Message]) -> SqlResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        // Ensure parent chat row exists (FK)
        {
            let conn = self.conn.lock().unwrap();
            let exists: Option<String> = conn
                .query_row(
                    "SELECT id FROM chats WHERE id=?1",
                    params![chat_id],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                conn.execute(
                    "INSERT OR IGNORE INTO chats (id, title, chat_type, unread_count, pinned, muted, archived, updated_at, raw_json)
                     VALUES (?1, NULL, 'unknown', 0, 0, 0, 0, ?2, ?3)",
                    params![chat_id, 0i64, "{}"],
                )?;
            }
        }

        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "
                INSERT INTO messages (id, chat_id, from_id, text, created_at, raw_json)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    from_id=excluded.from_id,
                    text=excluded.text,
                    created_at=excluded.created_at,
                    raw_json=excluded.raw_json
                ",
            )?;
            for msg in messages {
                let raw = serde_json::to_string(msg).unwrap_or_else(|_| "{}".into());
                stmt.execute(params![
                    msg.id,
                    chat_id,
                    msg.from_id,
                    msg.text,
                    msg.created.timestamp(),
                    raw,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_messages(&self, chat_id: &str, limit: Option<usize>) -> SqlResult<Vec<Message>> {
        let conn = self.conn.lock().unwrap();
        let limit = limit.unwrap_or(500) as i64;
        let mut stmt = conn.prepare(
            "SELECT raw_json FROM messages WHERE chat_id=?1 ORDER BY created_at ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![chat_id, limit], |row| {
            let raw: String = row.get(0)?;
            Ok(raw)
        })?;
        let mut out = Vec::new();
        for r in rows {
            let raw = r?;
            if let Ok(msg) = serde_json::from_str::<Message>(&raw) {
                out.push(msg);
            }
        }
        Ok(out)
    }

    pub fn clear_chat_messages(&self, chat_id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM messages WHERE chat_id=?1", params![chat_id])?;
        Ok(())
    }

    /// Legacy JSON-string API (compatibility).
    pub fn cache_chats(&self, chats_json: &str) -> SqlResult<()> {
        if let Ok(chats) = serde_json::from_str::<Vec<Chat>>(chats_json) {
            return self.upsert_chats(&chats);
        }
        Ok(())
    }

    pub fn get_cached_chats(&self) -> SqlResult<String> {
        let chats = self.get_chats()?;
        Ok(serde_json::to_string(&chats).unwrap_or_else(|_| "[]".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatType, MessageType};
    use chrono::Utc;
    use std::fs;

    fn temp_db() -> Database {
        let dir = std::env::temp_dir().join(format!("ym_db_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("cache.db");
        let conn = Connection::open(&path).unwrap();
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init_schema().unwrap();
        db
    }

    #[test]
    fn test_upsert_and_get_messages() {
        let db = temp_db();
        let chat = Chat {
            id: "c1".into(),
            title: Some("Test".into()),
            chat_type: ChatType::Private,
            rid: None,
            avatar_id: None,
            participants: vec![],
            unread_count: 1,
            last_message: None,
            pinned: false,
            archived: false,
            muted: false,
            created: None,
            updated: Some(Utc::now()),
        };
        db.upsert_chats(&[chat]).unwrap();

        let msg = Message {
            id: "m1".into(),
            chat_id: "c1".into(),
            from_id: "u1".into(),
            message_id: Some("m1".into()),
            rmid: None,
            type_: MessageType::Text,
            text: Some("hi".into()),
            entities: vec![],
            reply_to: None,
            forward: None,
            media: vec![],
            reactions: vec![],
            thread_id: None,
            has_thread: false,
            pinned: false,
            edited: false,
            edited_at: None,
            sent: true,
            delivered: true,
            read: false,
            created: Utc::now(),
            updated: None,
            poll: None,
        };
        db.upsert_messages("c1", &[msg]).unwrap();
        let loaded = db.get_messages("c1", None).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text.as_deref(), Some("hi"));
        let chats = db.get_chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!(chats[0].id, "c1");
    }
}
