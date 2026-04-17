use chrono::{DateTime, Utc};
use eremite_inference::ChatMessage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            created_at: Utc::now(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Convert to the inference crate's `ChatMessage` for generation calls.
    pub fn to_chat_message(&self) -> ChatMessage {
        ChatMessage::new(&self.role, &self.content)
    }
}

/// A conversation with a sequence of messages.
#[derive(Debug, Clone)]
pub struct Conversation {
    id: ConversationId,
    title: Option<String>,
    system_prompt: Option<String>,
    messages: Vec<Message>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new(system_prompt: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ConversationId::new(),
            title: None,
            system_prompt,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn id(&self) -> ConversationId {
        self.id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.updated_at = Utc::now();
    }

    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Build the full list of `ChatMessage`s for an inference call, including
    /// the system prompt (if any) as the first message.
    pub fn to_chat_messages(&self) -> Vec<ChatMessage> {
        let mut chat_messages = Vec::with_capacity(self.messages.len() + 1);
        if let Some(prompt) = &self.system_prompt {
            chat_messages.push(ChatMessage::system(prompt));
        }
        for msg in &self.messages {
            chat_messages.push(msg.to_chat_message());
        }
        chat_messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_id_is_unique() {
        let a = ConversationId::new();
        let b = ConversationId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn conversation_id_display() {
        let id = ConversationId::new();
        let s = id.to_string();
        assert!(!s.is_empty());
    }

    #[test]
    fn message_constructors() {
        let user = Message::user("hello");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, "hello");

        let assistant = Message::assistant("hi there");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.content, "hi there");
    }

    #[test]
    fn message_to_chat_message() {
        let msg = Message::user("test");
        let chat_msg = msg.to_chat_message();
        assert_eq!(chat_msg.role, "user");
        assert_eq!(chat_msg.content, "test");
    }

    #[test]
    fn conversation_new_without_system_prompt() {
        let conv = Conversation::new(None);
        assert!(conv.system_prompt().is_none());
        assert!(conv.title().is_none());
        assert!(conv.messages().is_empty());
    }

    #[test]
    fn conversation_new_with_system_prompt() {
        let conv = Conversation::new(Some("Be helpful.".to_string()));
        assert_eq!(conv.system_prompt(), Some("Be helpful."));
    }

    #[test]
    fn conversation_add_messages() {
        let mut conv = Conversation::new(None);
        conv.add_message(Message::user("hello"));
        conv.add_message(Message::assistant("hi"));
        assert_eq!(conv.messages().len(), 2);
        assert_eq!(conv.messages()[0].role, "user");
        assert_eq!(conv.messages()[1].role, "assistant");
    }

    #[test]
    fn conversation_set_title() {
        let mut conv = Conversation::new(None);
        assert!(conv.title().is_none());
        conv.set_title("My Chat");
        assert_eq!(conv.title(), Some("My Chat"));
    }

    #[test]
    fn to_chat_messages_includes_system_prompt() {
        let mut conv = Conversation::new(Some("You are helpful.".to_string()));
        conv.add_message(Message::user("hello"));

        let messages = conv.to_chat_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "You are helpful.");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[1].content, "hello");
    }

    #[test]
    fn to_chat_messages_without_system_prompt() {
        let mut conv = Conversation::new(None);
        conv.add_message(Message::user("hello"));

        let messages = conv.to_chat_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }
}
