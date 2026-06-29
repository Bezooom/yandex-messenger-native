use serde::{Deserialize, Serialize};

/// Стикеры — единица контента в стикерпаке.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Sticker {
    pub sticker_id: String,
    pub pack_id: String,
    pub file_url: String,
    pub thumb_url: String,
    pub width: u32,
    pub height: u32,
    pub emoji: String,
    pub file_size: u64,
    pub is_animated: bool,
    pub is_text_sticker: bool,
    pub text: Option<String>,
}

/// Обёртка для стикера-текста.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextStickerPayload {
    pub text: String,
    pub emoji: String,
    pub color: String,
}

/// Контейнер списка стикерпаков.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StickerPackList {
    pub packs: Vec<StickerPack>,
    pub next_cursor: Option<String>,
}

/// Стикерапк — коллекция стикеров.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StickerPack {
    pub pack_id: String,
    pub title: String,
    pub stickers: Vec<Sticker>,
    pub is_installed: bool,
    pub is_featured: bool,
    pub category: String,
    pub thumb_url: String,
    pub sticker_count: u32,
}

// === Методы StickerPack ===

impl StickerPack {
    /// Количество стикеров в паке.
    pub fn sticker_count(&self) -> usize {
        self.sticker_count as usize
    }

    /// Является ли папк избранным.
    pub fn is_featured(&self) -> bool {
        self.is_featured
    }

    /// Установлен ли папк у пользователя.
    pub fn is_installed(&self) -> bool {
        self.is_installed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sticker_pack_serialization() {
        let pack = StickerPack {
            pack_id: "abc123".to_string(),
            title: "Test".to_string(),
            stickers: vec![Sticker {
                sticker_id: "s1".to_string(),
                pack_id: "abc123".to_string(),
                file_url: "https://example.com/file.webp".to_string(),
                thumb_url: "https://example.com/thumb.webp".to_string(),
                width: 512,
                height: 512,
                emoji: "😀".to_string(),
                file_size: 10240,
                is_animated: false,
                is_text_sticker: false,
                text: None,
            }],
            is_installed: true,
            is_featured: true,
            category: "Emojis".to_string(),
            thumb_url: "https://example.com/pack-thumb.webp".to_string(),
            sticker_count: 1,
        };

        let json = serde_json::to_string(&pack).expect("serialize pack");
        assert!(json.contains("\"packId\":\"abc123\""));
        assert!(json.contains("\"title\":\"Test\""));
        assert!(json.contains("\"stickerCount\":1"));

        let deserialized: StickerPack = serde_json::from_str(&json).expect("deserialize pack");
        assert_eq!(deserialized.pack_id, "abc123");
        assert_eq!(deserialized.title, "Test");
        assert_eq!(deserialized.sticker_count(), 1);
        assert!(deserialized.is_featured());
        assert!(deserialized.is_installed());
    }

    #[test]
    fn test_text_sticker_payload() {
        let payload = TextStickerPayload {
            text: "Hello".to_string(),
            emoji: "👋".to_string(),
            color: "#FF5733".to_string(),
        };

        let json = serde_json::to_string(&payload).expect("serialize payload");
        let deserialized: TextStickerPayload =
            serde_json::from_str(&json).expect("deserialize payload");

        assert_eq!(deserialized.text, "Hello");
        assert_eq!(deserialized.emoji, "👋");
        assert_eq!(deserialized.color, "#FF5733");
    }

    #[test]
    fn test_sticker_pack_list() {
        let list = StickerPackList {
            packs: vec![],
            next_cursor: Some("next".to_string()),
        };

        let json = serde_json::to_string(&list).expect("serialize list");
        let deserialized: StickerPackList = serde_json::from_str(&json).expect("deserialize list");

        assert!(deserialized.next_cursor.is_some());
        assert_eq!(deserialized.next_cursor.unwrap(), "next");
    }
}
