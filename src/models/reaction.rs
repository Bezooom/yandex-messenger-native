use serde::{Deserialize, Serialize};

/// Extended reaction type with emoji metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedReaction {
    pub id: String,
    pub emoji: String,
    pub category: String,
    pub is_animated: bool,
}

/// Constraints on reactions for a chat or message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionRestrictions {
    pub max_reactions_per_message: u32,
    pub allow_custom_emojis: bool,
    pub allow_extended: bool,
}

impl Default for ReactionRestrictions {
    fn default() -> Self {
        Self {
            max_reactions_per_message: 10,
            allow_custom_emojis: false,
            allow_extended: false,
        }
    }
}

/// Full extended reactions configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedReactionsConfig {
    pub reactions: Vec<ExtendedReaction>,
    pub restrictions: ReactionRestrictions,
}
