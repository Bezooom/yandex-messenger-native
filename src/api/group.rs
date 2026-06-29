use crate::api::HttpClient;
use crate::models::group::*;
use crate::models::Chat;

impl HttpClient {
    /// Create a new group chat
    pub async fn create_group(
        &self,
        title: &str,
        members: Vec<String>,
        is_public: bool,
    ) -> Result<Chat, String> {
        let params = serde_json::json!({
            "title": title,
            "members": members,
            "is_public": is_public,
        });

        let data = self.rpc_request("create_group", params).await?;

        // Try to parse as Chat
        if let Ok(chat) = serde_json::from_value::<Chat>(data.clone()) {
            return Ok(chat);
        }

        // Fallback: construct Chat from response data
        let chat_id = data
            .get("chat_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Chat {
            id: chat_id,
            title: Some(title.to_string()),
            rid: None,
            chat_type: crate::models::ChatType::Group,
            avatar_id: None,
            participants: vec![],
            unread_count: 0,
            last_message: None,
            pinned: false,
            archived: false,
            muted: false,
            created: None,
            updated: None,
        })
    }

    /// Create a new channel
    pub async fn create_channel(
        &self,
        title: &str,
        description: Option<String>,
        is_public: bool,
    ) -> Result<Chat, String> {
        let params = serde_json::json!({
            "title": title,
            "description": description,
            "is_public": is_public,
        });

        let data = self.rpc_request("create_channel", params).await?;

        if let Ok(chat) = serde_json::from_value::<Chat>(data.clone()) {
            return Ok(chat);
        }

        let chat_id = data
            .get("chat_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(Chat {
            id: chat_id,
            title: Some(title.to_string()),
            rid: None,
            chat_type: crate::models::ChatType::Channel,
            avatar_id: None,
            participants: vec![],
            unread_count: 0,
            last_message: None,
            pinned: false,
            archived: false,
            muted: false,
            created: None,
            updated: None,
        })
    }

    /// Get group information
    pub async fn get_group_info(&self, chat_id: &str) -> Result<GroupSettings, String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
        });

        let data = self.rpc_request("get_group_info", params).await?;

        serde_json::from_value(data).map_err(|e| format!("Failed to parse group info: {}", e))
    }

    /// Get channel information
    pub async fn get_channel_info(&self, chat_id: &str) -> Result<ChannelSettings, String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
        });

        let data = self.rpc_request("get_channel_info", params).await?;

        serde_json::from_value(data).map_err(|e| format!("Failed to parse channel info: {}", e))
    }

    /// Get group members
    pub async fn get_group_members(
        &self,
        chat_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<GroupMember>, String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "limit": limit,
            "offset": offset,
        });

        let data = self.rpc_request("get_group_members", params).await?;

        if let Some(items) = data.get("members").and_then(|v| v.as_array()) {
            let mut members = Vec::new();
            for item in items {
                if let Ok(member) = serde_json::from_value::<GroupMember>(item.clone()) {
                    members.push(member);
                }
            }
            return Ok(members);
        }

        Ok(Vec::new())
    }

    /// Add a member to a group
    pub async fn add_group_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("add_group_member", params).await?;
        Ok(())
    }

    /// Remove a member from a group
    pub async fn remove_group_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("remove_group_member", params).await?;
        Ok(())
    }

    /// Update group settings
    pub async fn update_group_settings(
        &self,
        _chat_id: &str,
        settings: GroupSettings,
    ) -> Result<(), String> {
        let params = serde_json::to_value(settings).map_err(|e| e.to_string())?;

        let _data = self.rpc_request("update_group_settings", params).await?;
        Ok(())
    }

    /// Update channel settings
    pub async fn update_channel_settings(
        &self,
        _chat_id: &str,
        settings: ChannelSettings,
    ) -> Result<(), String> {
        let params = serde_json::to_value(settings).map_err(|e| e.to_string())?;

        let _data = self.rpc_request("update_channel_settings", params).await?;
        Ok(())
    }

    /// Generate an invite link for a group
    pub async fn generate_invite_link(&self, chat_id: &str) -> Result<String, String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
        });

        let data = self.rpc_request("generate_invite_link", params).await?;

        data.get("invite_link")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "No invite link in response".to_string())
    }

    /// Join a channel
    pub async fn join_channel(&self, chat_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
        });

        let _data = self.rpc_request("join_channel", params).await?;
        Ok(())
    }

    /// Leave a group
    pub async fn leave_group(&self, chat_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
        });

        let _data = self.rpc_request("leave_group", params).await?;
        Ok(())
    }

    /// Promote a member to admin
    pub async fn promote_to_admin(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("promote_to_admin", params).await?;
        Ok(())
    }

    /// Demote an admin to member
    pub async fn demote_from_admin(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("demote_from_admin", params).await?;
        Ok(())
    }

    /// Ban a member from the group
    pub async fn ban_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("ban_member", params).await?;
        Ok(())
    }

    /// Unban a member from the group
    pub async fn unban_member(&self, chat_id: &str, user_id: &str) -> Result<(), String> {
        let params = serde_json::json!({
            "chat_id": chat_id,
            "user_id": user_id,
        });

        let _data = self.rpc_request("unban_member", params).await?;
        Ok(())
    }
}
