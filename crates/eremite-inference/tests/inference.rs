use std::path::PathBuf;

use eremite_inference::{ChatMessage, InferenceEngine, InferenceEvent, InferenceParams};

fn test_model_path() -> PathBuf {
    std::env::var("EREMITE_TEST_MODEL")
        .map(PathBuf::from)
        .expect("set EREMITE_TEST_MODEL to a GGUF file path to run inference tests")
}

#[test]
#[ignore]
fn load_model_and_read_metadata() {
    let params = InferenceParams::default();
    let engine = InferenceEngine::load(test_model_path(), &params).unwrap();
    let meta = engine.model_metadata();

    assert!(!meta.description.is_empty());
    assert!(meta.n_params > 0);
    assert!(meta.n_ctx_train > 0);
}

#[test]
#[ignore]
fn generate_produces_tokens() {
    let params = InferenceParams {
        max_tokens: 32,
        temperature: 0.0,
        ..InferenceParams::default()
    };

    let mut engine = InferenceEngine::load(test_model_path(), &params).unwrap();
    let mut events: Vec<InferenceEvent> = Vec::new();

    let output = engine
        .generate("The capital of France is", &params, |e| {
            events.push(e);
        })
        .unwrap();

    assert!(!output.is_empty(), "expected non-empty output");
    assert!(
        events.iter().any(|e| matches!(e, InferenceEvent::Token(_))),
        "expected at least one Token event"
    );
    assert!(
        events
            .last()
            .is_some_and(|e| matches!(e, InferenceEvent::Done { .. })),
        "expected Done as final event"
    );
}

#[test]
#[ignore]
fn generate_chat_applies_template() {
    let params = InferenceParams {
        max_tokens: 32,
        temperature: 0.0,
        ..InferenceParams::default()
    };

    let mut engine = InferenceEngine::load(test_model_path(), &params).unwrap();
    let messages = vec![
        ChatMessage::system("You are a helpful assistant."),
        ChatMessage::user("What is 2 + 2?"),
    ];

    let output = engine
        .generate_chat(&messages, &params, |_| {})
        .unwrap();

    assert!(!output.is_empty(), "expected non-empty chat output");
}
