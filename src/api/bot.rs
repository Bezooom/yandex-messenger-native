#![allow(dead_code)]

use serde_json::Value;

use crate::models::bot::{BotInfo, BotCommand, BotMessage, InlineCallback, BotStartResult};

/// Bot API methods
impl crate::api::HttpClient {
    /// Получить информацию о боте
    pub async fn get_bot_info(&self, bot_id: &str) -> Result<BotInfo, String> {
        let params = serde_json::json!({
            "botId": bot_id
        });
        let data = self.rpc_request("get_bot", params).await?;
        let bot: BotInfo = serde_json::from_value(data).map_err(|e| format!("Failed to parse bot info: {}", e))?;
        Ok(bot)
    }

    /// Получить команды бота
    pub async fn get_bot_commands(&self, bot_id: &str) -> Result<Vec<BotCommand>, String> {
        let params = serde_json::json!({
            "botId": bot_id
        });
        let data = self.rpc_request("get_bot_commands", params).await?;
        let commands: Vec<BotCommand> = serde_json::from_value(data).map_err(|e| format!("Failed to parse bot commands: {}", e))?;
        Ok(commands)
    }

    /// Отправить команду боту
    pub async fn send_bot_command(
        &self,
        bot_id: &str,
        command: &str,
        params: Value,
    ) -> Result<BotMessage, String> {
        let chat_id = self.get_bot_chat_id(bot_id).await;
        let params = serde_json::json!({
            "chat_id": chat_id,
            "text": format!("/{}", command),
            "bot_params": params
        });
        let data = self.rpc_request("send_message", params).await?;
        let bot_msg: BotMessage = serde_json::from_value(data).map_err(|e| format!("Failed to parse bot message: {}", e))?;
        Ok(bot_msg)
    }

    /// Отправить inline callback
    pub async fn send_inline_callback(
        &self,
        bot_id: &str,
        callback_data: &str,
    ) -> Result<InlineCallback, String> {
        let params = serde_json::json!({
            "botId": bot_id,
            "callbackData": callback_data
        });
        let data = self.rpc_request("inline_callback", params).await?;
        let callback: InlineCallback = serde_json::from_value(data).map_err(|e| format!("Failed to parse inline callback: {}", e))?;
        Ok(callback)
    }

    /// Запустить бота с параметром
    pub async fn start_bot(
        &self,
        bot_id: &str,
        start_param: &str,
    ) -> Result<BotStartResult, String> {
        let params = serde_json::json!({
            "botId": bot_id,
            "startParam": start_param
        });
        let data = self.rpc_request("start_bot", params).await?;
        let result: BotStartResult = serde_json::from_value(data).map_err(|e| format!("Failed to parse bot start result: {}", e))?;
        Ok(result)
    }

    /// Получить сообщения бота
    pub async fn get_bot_messages(
        &self,
        bot_id: &str,
        limit: usize,
    ) -> Result<Vec<BotMessage>, String> {
        let params = serde_json::json!({
            "botId": bot_id,
            "limit": limit
        });
        let data = self.rpc_request("get_bot_messages", params).await?;
        let messages: Vec<BotMessage> = serde_json::from_value(data).map_err(|e| format!("Failed to parse bot messages: {}", e))?;
        Ok(messages)
    }

    /// Получить/создать чат с ботом
    pub async fn get_bot_chat_id(&self, bot_id: &str) -> String {
        format!("bot_{}", bot_id)
    }
}

/// WebSocket bot methods
impl crate::api::WebSocketClient {
    /// Подписаться на обновления бота
    pub async fn subscribe_bot(&self, bot_id: &str) -> Result<u64, String> {
        self.send_message("subscribe_bot", serde_json::json!({ "botId": bot_id })).await
    }

    /// Отправить callback боту через WebSocket
    pub async fn send_bot_callback(&self, bot_id: &str, callback_data: &str) -> Result<u64, String> {
        self.send_message("bot_callback", serde_json::json!({
            "botId": bot_id,
            "callbackData": callback_data
        })).await
    }

    /// Отправить команду боту через WebSocket
    pub async fn send_bot_command_ws(&self, bot_id: &str, command: &str) -> Result<u64, String> {
        self.send_message("bot_command", serde_json::json!({
            "botId": bot_id,
            "command": command
        })).await
    }
}
