use axga_core::Tool;
use axga_core::session::load_session;
use axga_core::tools::code::{DiffTool, GrepTool};
use axga_shared::error::AxgaError;
use axga_shared::limits;
use serde_json::json;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "axga-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn oversized_file(name: &str) -> PathBuf {
    let path = temp_path(name);
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(limits::MAX_FILE_READ_SIZE + 1).unwrap();
    path
}

#[tokio::test]
async fn diff_rejects_oversized_files() {
    let large = oversized_file("diff-large.txt");
    let small = temp_path("diff-small.txt");
    std::fs::write(&small, "small\n").unwrap();

    let result = DiffTool
        .execute(json!({
            "path_a": large.display().to_string(),
            "path_b": small.display().to_string(),
        }))
        .await;

    let _ = std::fs::remove_file(&large);
    let _ = std::fs::remove_file(&small);

    assert!(matches!(result, Err(AxgaError::FileTooLarge { .. })));
}

#[tokio::test]
async fn grep_skips_oversized_files() {
    let large = oversized_file("grep-large.txt");

    let output = GrepTool
        .execute(json!({
            "pattern": "anything",
            "path": large.display().to_string(),
        }))
        .await
        .unwrap();

    let _ = std::fs::remove_file(&large);

    assert_eq!(output, "No matches found.");
}

#[test]
fn load_session_rejects_oversized_file() {
    let large = oversized_file("session-large.jsonl");

    let result = load_session(&large);

    let _ = std::fs::remove_file(&large);

    assert!(result.is_err());
}
