/// Events emitted during inference, delivered via the `FnMut(InferenceEvent)`
/// callback passed to [`InferenceEngine::generate`](crate::InferenceEngine::generate)
/// and [`InferenceEngine::generate_chat`](crate::InferenceEngine::generate_chat).
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceEvent {
    /// A new token piece was decoded into text.
    Token(String),
    /// Generation finished, either by reaching the end-of-generation token
    /// or the `max_tokens` limit.
    Done {
        tokens_generated: u32,
        duration_ms: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_event_holds_text() {
        let event = InferenceEvent::Token("hello".to_string());
        assert_eq!(event, InferenceEvent::Token("hello".to_string()));
    }

    #[test]
    fn done_event_holds_stats() {
        let event = InferenceEvent::Done {
            tokens_generated: 42,
            duration_ms: 1000,
        };
        match event {
            InferenceEvent::Done {
                tokens_generated,
                duration_ms,
            } => {
                assert_eq!(tokens_generated, 42);
                assert_eq!(duration_ms, 1000);
            }
            _ => panic!("expected Done variant"),
        }
    }

    #[test]
    fn events_are_cloneable() {
        let original = InferenceEvent::Token("test".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
