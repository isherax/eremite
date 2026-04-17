use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use eremite_models::ModelManager;

const REPO_ID: &str = "test-org/test-model-GGUF";
const FILENAME: &str = "test-model-Q4_K_M.gguf";
const FAKE_MODEL_BYTES: &[u8] = b"fake gguf model content for testing";

fn expected_sha256() -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(FAKE_MODEL_BYTES);
    format!("{:x}", hasher.finalize())
}

async fn setup() -> (MockServer, TempDir, ModelManager) {
    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let manager = ModelManager::new(tmp.path()).unwrap();
    (server, tmp, manager)
}

async fn mount_success(server: &MockServer) -> wiremock::MockGuard {
    Mock::given(method("GET"))
        .and(path(format!("/{REPO_ID}/resolve/main/{FILENAME}")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(FAKE_MODEL_BYTES))
        .mount_as_scoped(server)
        .await
}

#[tokio::test]
async fn download_stores_file_and_manifest() {
    let (server, tmp, mut manager) = setup().await;
    let _guard = mount_success(&server).await;

    let entry = manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();

    assert_eq!(entry.repo_id, REPO_ID);
    assert_eq!(entry.filename, FILENAME);
    assert_eq!(entry.size_bytes, FAKE_MODEL_BYTES.len() as u64);
    assert_eq!(entry.sha256, expected_sha256());

    let file_path = manager.model_path(REPO_ID, FILENAME);
    assert!(file_path.exists());
    let contents = std::fs::read(&file_path).unwrap();
    assert_eq!(contents, FAKE_MODEL_BYTES);

    let manifest_path = tmp.path().join("models").join("manifest.json");
    assert!(manifest_path.exists());
}

#[tokio::test]
async fn list_after_download() {
    let (server, _tmp, mut manager) = setup().await;
    let _guard = mount_success(&server).await;

    assert!(manager.list().is_empty());

    manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();

    let models = manager.list();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].repo_id, REPO_ID);
    assert_eq!(models[0].filename, FILENAME);
}

#[tokio::test]
async fn get_finds_downloaded_model() {
    let (server, _tmp, mut manager) = setup().await;
    let _guard = mount_success(&server).await;

    manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();

    let found = manager.get(REPO_ID, FILENAME);
    assert!(found.is_some());
    assert_eq!(found.unwrap().sha256, expected_sha256());

    let not_found = manager.get("nonexistent/repo", "no-file.gguf");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn remove_deletes_file_and_manifest_entry() {
    let (server, _tmp, mut manager) = setup().await;
    let _guard = mount_success(&server).await;

    manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();

    let file_path = manager.model_path(REPO_ID, FILENAME);
    assert!(file_path.exists());

    manager.remove(REPO_ID, FILENAME).unwrap();

    assert!(!file_path.exists());
    assert!(manager.list().is_empty());
    assert!(manager.get(REPO_ID, FILENAME).is_none());
}

#[tokio::test]
async fn download_same_model_twice_overwrites() {
    let (server, _tmp, mut manager) = setup().await;
    let _guard = mount_success(&server).await;

    manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();
    manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await
        .unwrap();

    assert_eq!(manager.list().len(), 1);

    let file_path = manager.model_path(REPO_ID, FILENAME);
    let contents = std::fs::read(&file_path).unwrap();
    assert_eq!(contents, FAKE_MODEL_BYTES);
}

#[tokio::test]
async fn download_404_returns_error() {
    let (server, _tmp, mut manager) = setup().await;

    Mock::given(method("GET"))
        .and(path(format!("/{REPO_ID}/resolve/main/{FILENAME}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let result = manager
        .download(REPO_ID, FILENAME, Some(&server.uri()))
        .await;

    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("404"), "error should mention 404: {err_msg}");
}
