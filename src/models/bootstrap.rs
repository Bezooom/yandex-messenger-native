//! Bootstrap chat list parsing.
//!
//! Field names come from a captured `bootstrap` RPC payload
//! (`chats[].members_count`, `chats[].private`, buckets `pinned_chats` and
//! `chat_mutings`). The previous parser dropped those fields and invented
//! "Группа • 0 участников".

use chrono::Utc;
use serde_json::Value;

use super::{Chat, ChatType, Message, MessageType, Participant};

/// Sidebar folders observed in the official desktop rail and web folder tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatListFilter {
    #[default]
    All,
    /// `included_type_ids: ["private"]` — Личные
    Private,
    /// `included_type_ids: ["unread"]` — Новые
    Unread,
}

pub fn matches_list_filter(chat: &Chat, filter: ChatListFilter) -> bool {
    match filter {
        ChatListFilter::All => true,
        ChatListFilter::Private => matches!(chat.chat_type, ChatType::Private | ChatType::Bot),
        ChatListFilter::Unread => chat.unread_count > 0,
    }
}

/// OS window title. Official desktop: `Яндекс Телемост — 1 новое сообщение`.
pub fn window_title(unread: u32) -> String {
    if unread == 0 {
        return "Яндекс Телемост".to_string();
    }
    format!(
        "Яндекс Телемост — {}",
        ru_count(
            unread,
            "новое сообщение",
            "новых сообщения",
            "новых сообщений"
        )
    )
}

pub fn ru_count(n: u32, one: &str, few: &str, many: &str) -> String {
    let n10 = n % 10;
    let n100 = n % 100;
    let word = if (11..15).contains(&n100) {
        many
    } else if n10 == 1 {
        one
    } else if (2..5).contains(&n10) {
        few
    } else {
        many
    };
    format!("{n} {word}")
}

/// Header subtitle. Never claims a member count or presence we do not have.
pub fn chat_status_line(chat: &Chat) -> String {
    match chat.chat_type {
        ChatType::Private => {
            if chat.title.as_deref() == Some("Избранное") {
                "Избранное".to_string()
            } else {
                String::new()
            }
        }
        ChatType::Bot => "бот".to_string(),
        ChatType::Group => member_line(chat, "участник", "участника", "участников", "Группа"),
        ChatType::Channel => member_line(chat, "подписчик", "подписчика", "подписчиков", "Канал"),
        ChatType::Unknown => String::new(),
    }
}

fn member_line(chat: &Chat, one: &str, few: &str, many: &str, fallback: &str) -> String {
    let n = if chat.member_count > 0 {
        chat.member_count
    } else {
        chat.participants.len() as u32
    };
    if n == 0 {
        fallback.to_string()
    } else {
        ru_count(n, one, few, many)
    }
}

/// Parse the `bootstrap` RPC body into chats.
///
/// `private: true` and `guid_guid` ids are personal chats. Groups keep
/// `members_count` instead of an empty participant vec. Pin and mute come
/// from the `pinned_chats` and `chat_mutings` buckets.
pub fn parse_bootstrap_chats(data: &Value) -> Vec<Chat> {
    let current_user_id = data
        .get("user")
        .and_then(|u| u.get("guid"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut names: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut avatars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut robots: std::collections::HashSet<String> = std::collections::HashSet::new();

    if let Some(user) = data.get("user") {
        remember_person(user, &mut names, &mut avatars, &mut robots, false);
    }
    if let Some(arr) = data.get("users").and_then(|v| v.as_array()) {
        for person in arr {
            remember_person(person, &mut names, &mut avatars, &mut robots, false);
        }
    }
    if let Some(arr) = data.get("contacts").and_then(|v| v.as_array()) {
        for person in arr {
            remember_person(person, &mut names, &mut avatars, &mut robots, true);
        }
    }

    let mut pinned = std::collections::HashSet::new();
    let mut muted = std::collections::HashSet::new();
    if let Some(buckets) = data.get("buckets").and_then(|v| v.as_array()) {
        for bucket in buckets {
            let name = bucket
                .get("bucket_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let value = bucket.get("bucket_value");
            match name {
                "pinned_chats" => {
                    if let Some(ids) = value
                        .and_then(|v| v.get("pinned_chats"))
                        .and_then(|v| v.as_array())
                    {
                        for id in ids {
                            if let Some(id) = id.as_str() {
                                pinned.insert(id.to_string());
                            }
                        }
                    }
                }
                "chat_mutings" => {
                    if let Some(obj) = value.and_then(|v| v.as_object()) {
                        for (id, flags) in obj {
                            if flags.get("mute").and_then(|v| v.as_bool()).unwrap_or(false) {
                                muted.insert(id.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut recent_text: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Some(messages) = data.get("messages").and_then(|v| v.as_array()) {
        for message in messages {
            let chat_id = message
                .get("server_message_info")
                .and_then(|info| info.get("chat_id"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    message
                        .get("ServerMessageInfo")
                        .and_then(|info| info.get("ChatId"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");
            if chat_id.is_empty() {
                continue;
            }
            if let Some(text) = message_text(message) {
                recent_text.insert(chat_id.to_string(), text);
            }
        }
    }

    let Some(chats) = data.get("chats").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut out = Vec::with_capacity(chats.len());
    for raw in chats {
        let id = raw
            .get("chat_id")
            .or_else(|| raw.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }

        let explicit_name = raw
            .get("name")
            .or_else(|| raw.get("title"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let is_private = raw
            .get("private")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || is_guid_pair(&id);

        let (title, avatar_id, chat_type) = if is_private {
            let clean = id.rsplit('/').next().unwrap_or(&id);
            let parts: Vec<&str> = clean.split('_').collect();
            if parts.len() == 2 && parts[0] == parts[1] {
                (
                    Some("Избранное".to_string()),
                    avatars.get(current_user_id).cloned(),
                    ChatType::Private,
                )
            } else {
                let other = parts.iter().copied().find(|p| *p != current_user_id);
                let other = other.unwrap_or("");
                let robot = robots.contains(other);
                let title = explicit_name.or_else(|| names.get(other).cloned());
                let avatar = raw
                    .get("avatar_id")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| avatars.get(other).cloned());
                let kind = if robot {
                    ChatType::Bot
                } else {
                    ChatType::Private
                };
                (title, avatar, kind)
            }
        } else {
            let avatar = raw
                .get("avatar_id")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            (explicit_name, avatar, ChatType::Group)
        };

        let listed = raw
            .get("members_count")
            .or_else(|| raw.get("member_count"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let member_ids: Vec<String> = raw
            .get("members")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let participants: Vec<Participant> = member_ids
            .iter()
            .map(|id| Participant {
                id: id.clone(),
                name: names.get(id).cloned(),
                avatar: avatars.get(id).cloned(),
                status: None,
                last_seen: None,
            })
            .collect();
        let member_count = if listed > 0 {
            listed
        } else {
            participants.len() as u32
        };
        let created = timestamp_from(raw.get("create_timestamp"));
        let updated = timestamp_from(raw.get("version")).or(created);
        let stamp = updated.or(created);
        let text = raw
            .get("last_message")
            .and_then(message_text)
            .or_else(|| recent_text.get(&id).cloned());
        let last_message = text.map(|final_text| Message {
            id: String::new(),
            chat_id: id.clone(),
            from_id: String::new(),
            message_id: None,
            rmid: None,
            type_: MessageType::Text,
            text: Some(final_text),
            created: stamp.unwrap_or_else(Utc::now),
            updated: None,
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
            read: true,
            poll: None,
        });

        let inline_pin = raw.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false)
            || raw
                .get("is_pinned")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        out.push(Chat {
            id: id.clone(),
            title,
            rid: None,
            chat_type,
            avatar_id,
            participants,
            member_count,
            unread_count: 0,
            last_message,
            pinned: inline_pin || pinned.contains(&id),
            archived: false,
            muted: muted.contains(&id),
            created,
            updated,
        });
    }
    out
}

fn is_guid_pair(id: &str) -> bool {
    let clean = id.rsplit('/').next().unwrap_or(id);
    let mut parts = clean.split('_');
    let Some(a) = parts.next() else {
        return false;
    };
    let Some(b) = parts.next() else {
        return false;
    };
    parts.next().is_none() && looks_like_guid(a) && looks_like_guid(b)
}

fn looks_like_guid(s: &str) -> bool {
    s.len() >= 32 && s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// `create_timestamp` is unix seconds; `version` in captures is microseconds.
fn timestamp_from(value: Option<&Value>) -> Option<chrono::DateTime<Utc>> {
    let n = value?
        .as_i64()
        .or_else(|| value?.as_u64().and_then(|u| i64::try_from(u).ok()))?;
    let secs = if n.abs() > 1_000_000_000_000 {
        n / 1_000_000
    } else {
        n
    };
    chrono::DateTime::from_timestamp(secs, 0)
}

fn remember_person(
    obj: &Value,
    names: &mut std::collections::HashMap<String, String>,
    avatars: &mut std::collections::HashMap<String, String>,
    robots: &mut std::collections::HashSet<String>,
    overwrite: bool,
) {
    let Some(guid) = obj.get("guid").and_then(|v| v.as_str()) else {
        return;
    };
    if guid.is_empty() {
        return;
    }
    if let Some(name) = display_name(obj) {
        if overwrite {
            names.insert(guid.to_string(), name);
        } else {
            names.entry(guid.to_string()).or_insert(name);
        }
    }
    if let Some(avatar) = obj.get("avatar_id").and_then(|v| v.as_str()) {
        avatars
            .entry(guid.to_string())
            .or_insert(avatar.to_string());
    }
    if obj
        .get("is_robot")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        robots.insert(guid.to_string());
    }
}

fn display_name(obj: &Value) -> Option<String> {
    for key in ["contact_name", "display_name", "public_name", "name"] {
        if let Some(name) = obj.get(key).and_then(|v| v.as_str()) {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn message_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(text) = value.get("message").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    value
        .get("ClientMessage")
        .and_then(|m| m.get("Plain"))
        .and_then(|p| p.get("Text"))
        .and_then(|t| t.get("MessageText"))
        .and_then(|t| t.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            value
                .get("message")
                .and_then(|m| m.get("text"))
                .and_then(|t| t.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "user": { "guid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "display_name": "Я" },
            "users": [
                { "guid": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "display_name": "Дока" },
                { "guid": "cccccccc-cccc-cccc-cccc-cccccccccccc", "display_name": "Афиша", "is_robot": true }
            ],
            "buckets": [
                {
                    "bucket_name": "pinned_chats",
                    "bucket_value": { "pinned_chats": ["0/0/group-1"] }
                },
                {
                    "bucket_name": "chat_mutings",
                    "bucket_value": {
                        "0/22/meet-1": { "mute": true, "mute_mentions": false }
                    }
                }
            ],
            "chats": [
                {
                    "chat_id": "0/0/group-1",
                    "name": "Дока",
                    "members_count": 2,
                    "members": ["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"]
                },
                {
                    "chat_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "private": true,
                    "members_count": 2
                },
                {
                    "chat_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "private": true,
                    "members_count": 1
                },
                {
                    "chat_id": "cccccccc-cccc-cccc-cccc-cccccccccccc_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                    "private": true,
                    "members_count": 2
                },
                {
                    "chat_id": "0/22/meet-1",
                    "name": "Встреча в Телемосте",
                    "members_count": 2
                }
            ]
        })
    }

    #[test]
    fn parses_member_count_pin_mute_and_private() {
        let chats = parse_bootstrap_chats(&sample());
        let group = chats.iter().find(|c| c.id == "0/0/group-1").unwrap();
        assert_eq!(group.chat_type, ChatType::Group);
        assert_eq!(group.member_count, 2);
        assert!(group.pinned);
        assert_eq!(chat_status_line(group), "2 участника");
        assert!(!chat_status_line(group).contains('0'));
        assert_eq!(group.participants.len(), 2);
        assert!(group
            .participants
            .iter()
            .any(|p| p.name.as_deref() == Some("Дока")));

        let dm = chats.iter().find(|c| c.id.starts_with("bbbbbbbb")).unwrap();
        assert_eq!(dm.chat_type, ChatType::Private);
        assert_eq!(dm.title.as_deref(), Some("Дока"));
        assert_eq!(chat_status_line(dm), "");

        let saved = chats
            .iter()
            .find(|c| c.title.as_deref() == Some("Избранное"));
        assert!(saved.is_some());

        let bot = chats.iter().find(|c| c.chat_type == ChatType::Bot).unwrap();
        assert_eq!(bot.title.as_deref(), Some("Афиша"));
        assert_eq!(chat_status_line(bot), "бот");

        let meet = chats.iter().find(|c| c.id == "0/22/meet-1").unwrap();
        assert!(meet.muted);
        assert_eq!(meet.member_count, 2);
    }

    #[test]
    fn zero_members_does_not_claim_zero() {
        let chat = Chat {
            id: "0/0/x".into(),
            title: Some("Пусто".into()),
            rid: None,
            chat_type: ChatType::Group,
            avatar_id: None,
            participants: vec![],
            member_count: 0,
            unread_count: 0,
            last_message: None,
            pinned: false,
            archived: false,
            muted: false,
            created: None,
            updated: None,
        };
        assert_eq!(chat_status_line(&chat), "Группа");
    }

    #[test]
    fn window_title_matches_official_shape() {
        assert_eq!(window_title(0), "Яндекс Телемост");
        assert_eq!(window_title(1), "Яндекс Телемост — 1 новое сообщение");
        assert_eq!(window_title(2), "Яндекс Телемост — 2 новых сообщения");
        assert_eq!(window_title(5), "Яндекс Телемост — 5 новых сообщений");
        assert_eq!(window_title(21), "Яндекс Телемост — 21 новое сообщение");
    }

    #[test]
    fn list_filters_match_folder_types() {
        let mut chat = Chat {
            id: "dm".into(),
            title: None,
            rid: None,
            chat_type: ChatType::Private,
            avatar_id: None,
            participants: vec![],
            member_count: 2,
            unread_count: 3,
            last_message: None,
            pinned: false,
            archived: false,
            muted: false,
            created: None,
            updated: None,
        };
        assert!(matches_list_filter(&chat, ChatListFilter::Private));
        assert!(matches_list_filter(&chat, ChatListFilter::Unread));
        chat.chat_type = ChatType::Group;
        chat.unread_count = 0;
        assert!(!matches_list_filter(&chat, ChatListFilter::Private));
        assert!(!matches_list_filter(&chat, ChatListFilter::Unread));
        assert!(matches_list_filter(&chat, ChatListFilter::All));
    }

    #[test]
    fn contact_name_overrides_display_name() {
        let data = serde_json::json!({
            "user": { "guid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "display_name": "Я" },
            "users": [{ "guid": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "display_name": "Janina Davydova" }],
            "contacts": [{ "guid": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "contact_name": "Янина Качан" }],
            "chats": [{
                "chat_id": "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb_aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "private": true,
                "members": ["bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"]
            }]
        });
        let chats = parse_bootstrap_chats(&data);
        assert_eq!(chats[0].title.as_deref(), Some("Янина Качан"));
        let peer = chats[0]
            .participants
            .iter()
            .find(|p| p.id.starts_with("bbbb"))
            .unwrap();
        assert_eq!(peer.name.as_deref(), Some("Янина Качан"));
    }

    #[test]
    fn preview_timestamp_comes_from_version_not_now() {
        let data = serde_json::json!({
            "user": { "guid": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" },
            "chats": [
                {
                    "chat_id": "0/0/older",
                    "name": "Older",
                    "members_count": 2,
                    "create_timestamp": 1700000000,
                    "version": 1700000001000000u64,
                    "last_message": { "text": "a" }
                },
                {
                    "chat_id": "0/0/newer",
                    "name": "Newer",
                    "members_count": 2,
                    "create_timestamp": 1700000000,
                    "version": 1700000009000000u64,
                    "last_message": { "text": "b" }
                }
            ]
        });
        let chats = parse_bootstrap_chats(&data);
        let older = chats.iter().find(|c| c.id == "0/0/older").unwrap();
        let newer = chats.iter().find(|c| c.id == "0/0/newer").unwrap();
        let older_at = older.last_message.as_ref().unwrap().created;
        let newer_at = newer.last_message.as_ref().unwrap().created;
        assert!(newer_at > older_at);
        assert_eq!(older_at.timestamp(), 1700000001);
        assert_eq!(newer.updated.unwrap().timestamp(), 1700000009);
        assert!(newer_at.timestamp() < 1_800_000_000);
    }
}
