use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use eremite_models::{popular_gguf_models, search_gguf_models, SearchResult};

const SAMPLE_RESPONSE: &str = r#"[
  {
    "id": "demo/Example-GGUF",
    "author": "demo",
    "downloads": 1000,
    "likes": 42,
    "tags": ["gguf", "text-generation"],
    "siblings": [
      { "rfilename": "README.md" },
      { "rfilename": "model-Q4_K_M.gguf", "size": 12345 },
      { "rfilename": "model-Q8_0.gguf" }
    ]
  }
]"#;

#[tokio::test]
async fn search_parses_gguf_files_and_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/models"))
        .and(query_param("pipeline_tag", "text-generation"))
        .and(query_param("filter", "gguf"))
        .and(query_param("sort", "downloads"))
        .and(query_param("direction", "-1"))
        .and(query_param("limit", "10"))
        .and(query_param("full", "true"))
        .and(query_param("search", "llama"))
        .respond_with(ResponseTemplate::new(200).set_body_string(SAMPLE_RESPONSE))
        .mount(&server)
        .await;

    let origin = server.uri();
    let results: Vec<SearchResult> = search_gguf_models(&origin, "llama", 10)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].repo_id, "demo/Example-GGUF");
    assert_eq!(results[0].author.as_deref(), Some("demo"));
    assert_eq!(results[0].downloads, 1000);
    assert_eq!(results[0].likes, 42);
    assert_eq!(results[0].gguf_files.len(), 2);

    let q4 = results[0]
        .gguf_files
        .iter()
        .find(|f| f.filename == "model-Q4_K_M.gguf")
        .unwrap();
    assert_eq!(q4.size_bytes, Some(12345));
    assert_eq!(q4.quantization_label.as_deref(), Some("Q4_K_M"));

    let q8 = results[0]
        .gguf_files
        .iter()
        .find(|f| f.filename == "model-Q8_0.gguf")
        .unwrap();
    assert_eq!(q8.size_bytes, None);
    assert_eq!(q8.quantization_label.as_deref(), Some("Q8_0"));
}

#[tokio::test]
async fn popular_models_omits_search_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/models"))
        .and(query_param("pipeline_tag", "text-generation"))
        .and(query_param("filter", "gguf"))
        .and(query_param("sort", "downloads"))
        .and(query_param("direction", "-1"))
        .and(query_param("limit", "5"))
        .and(query_param("full", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .mount(&server)
        .await;

    let list = popular_gguf_models(&server.uri(), 5).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn search_http_error_is_reported() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/models"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = search_gguf_models(&server.uri(), "x", 5)
        .await
        .unwrap_err();
    let msg = format!("{err:#}");
    assert!(
        msg.contains("500") || msg.contains("Hub search failed"),
        "unexpected error: {msg}"
    );
}
