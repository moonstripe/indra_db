//! CLI Integration Tests
//!
//! These tests verify that the CLI commands work correctly end-to-end.
//! They test the actual binary behavior, not just the library.
//!
//! Run with:
//! ```bash
//! cargo test --test cli_integration
//! ```

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

/// Get the path to the built binary
fn indra_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target");
    path.push("debug");
    path.push("indra");
    path
}

/// Run indra command and return (stdout, stderr, success)
fn run_indra(args: &[&str], db_path: &str) -> (String, String, bool) {
    let output = Command::new(indra_binary())
        .args(["-d", db_path, "-f", "json", "--embedder", "mock"])
        .args(args)
        .output()
        .expect("Failed to execute indra");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

// ============================================================================
// Database Initialization Tests
// ============================================================================

#[test]
fn test_cli_init_creates_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    let (stdout, _stderr, success) = run_indra(&["init"], db_str);

    assert!(success, "init should succeed");
    assert!(stdout.contains("status"), "should return JSON with status");
    assert!(stdout.contains("ok"), "status should be ok");
    assert!(db_path.exists(), ".indra file should be created");
}

#[test]
fn test_cli_default_path_is_dot_indra() {
    // Verify the help text shows .indra as default
    let output = Command::new(indra_binary())
        .args(["--help"])
        .output()
        .expect("Failed to execute indra");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[default: .indra]"),
        "Default database path should be .indra, got: {}",
        stdout
    );
}

// ============================================================================
// Thought CRUD Tests
// ============================================================================

#[test]
fn test_cli_create_thought() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    // Init
    run_indra(&["init"], db_str);

    // Create thought
    let (stdout, _stderr, success) = run_indra(&["create", "Hello, world!"], db_str);

    assert!(success, "create should succeed");
    assert!(
        stdout.contains("\"status\":\"ok\""),
        "should return ok status"
    );
    assert!(stdout.contains("\"id\":"), "should return thought ID");
}

#[test]
fn test_cli_create_thought_with_custom_id() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    let (stdout, _stderr, success) = run_indra(
        &["create", "My custom thought", "--id", "my-custom-id"],
        db_str,
    );

    assert!(success, "create with custom ID should succeed");
    assert!(
        stdout.contains("\"id\":\"my-custom-id\""),
        "should use custom ID"
    );
}

#[test]
fn test_cli_get_thought() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "Test content", "--id", "test-id"], db_str);

    let (stdout, _stderr, success) = run_indra(&["get", "test-id"], db_str);

    assert!(success, "get should succeed");
    assert!(
        stdout.contains("\"content\":\"Test content\""),
        "should return correct content"
    );
    assert!(
        stdout.contains("\"id\":\"test-id\""),
        "should return correct ID"
    );
}

#[test]
fn test_cli_get_nonexistent_thought() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    let (_stdout, _stderr, success) = run_indra(&["get", "nonexistent"], db_str);

    assert!(!success, "get nonexistent should fail");
}

#[test]
fn test_cli_list_thoughts() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "First thought", "--id", "first"], db_str);
    run_indra(&["create", "Second thought", "--id", "second"], db_str);

    let (stdout, _stderr, success) = run_indra(&["list"], db_str);

    assert!(success, "list should succeed");
    assert!(stdout.contains("\"count\":2"), "should have 2 thoughts");
    assert!(
        stdout.contains("First thought"),
        "should contain first thought"
    );
    assert!(
        stdout.contains("Second thought"),
        "should contain second thought"
    );
}

#[test]
fn test_cli_update_thought() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(
        &["create", "Original content", "--id", "update-test"],
        db_str,
    );

    let (stdout, _stderr, success) =
        run_indra(&["update", "update-test", "Updated content"], db_str);

    assert!(success, "update should succeed");
    assert!(stdout.contains("\"status\":\"ok\""), "should return ok");

    // Verify the update
    let (stdout, _, _) = run_indra(&["get", "update-test"], db_str);
    assert!(
        stdout.contains("Updated content"),
        "content should be updated"
    );
}

#[test]
fn test_cli_delete_thought() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "To be deleted", "--id", "delete-me"], db_str);

    // Verify it exists
    let (stdout, _, success) = run_indra(&["get", "delete-me"], db_str);
    assert!(success, "thought should exist before delete");

    // Delete
    let (stdout, _stderr, success) = run_indra(&["delete", "delete-me"], db_str);
    assert!(success, "delete should succeed");
    assert!(stdout.contains("\"status\":\"ok\""), "should return ok");

    // Verify it's gone
    let (_, _, success) = run_indra(&["get", "delete-me"], db_str);
    assert!(!success, "thought should not exist after delete");
}

// ============================================================================
// Auto-commit Tests (Critical for MCP integration)
// ============================================================================

#[test]
fn test_cli_auto_commits_by_default() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(
        &["create", "Auto-committed thought", "--id", "auto-test"],
        db_str,
    );

    // Check that a commit was created
    let (stdout, _stderr, success) = run_indra(&["log"], db_str);

    assert!(success, "log should succeed");
    assert!(stdout.contains("\"count\":1"), "should have 1 commit");
    assert!(
        stdout.contains("Auto-commit"),
        "commit message should indicate auto-commit"
    );
}

#[test]
fn test_cli_no_auto_commit_flag() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    // Init first (without no-auto-commit since init doesn't support it)
    run_indra(&["init"], db_str);

    // Create with --no-auto-commit
    let output = Command::new(indra_binary())
        .args([
            "-d",
            db_str,
            "-f",
            "json",
            "--embedder",
            "mock",
            "--no-auto-commit",
        ])
        .args(["create", "Not auto-committed"])
        .output()
        .expect("Failed to execute indra");

    assert!(output.status.success(), "create should succeed");

    // Check that no commit was created (only the init creates the file structure)
    let (stdout, _stderr, _) = run_indra(&["log"], db_str);

    // With no-auto-commit, there should be 0 commits
    assert!(
        stdout.contains("\"count\":0") || stdout.contains("\"commits\":[]"),
        "should have 0 commits when using --no-auto-commit, got: {}",
        stdout
    );
}

// ============================================================================
// Persistence Tests (Critical - this was the bug!)
// ============================================================================

#[test]
fn test_cli_data_persists_across_invocations() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    // First session: create data
    run_indra(&["init"], db_str);
    run_indra(
        &["create", "Persistent thought 1", "--id", "persist-1"],
        db_str,
    );
    run_indra(
        &["create", "Persistent thought 2", "--id", "persist-2"],
        db_str,
    );

    // Simulate "new session" by just running more commands
    // (In reality, the MCP spawns a new process for each command)

    // Second "session": verify data exists
    let (stdout, _stderr, success) = run_indra(&["list"], db_str);

    assert!(success, "list should succeed in second session");
    assert!(
        stdout.contains("\"count\":2"),
        "should still have 2 thoughts"
    );
    assert!(stdout.contains("persist-1"), "should have first thought");
    assert!(stdout.contains("persist-2"), "should have second thought");
}

#[test]
fn test_cli_commits_persist_across_invocations() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    // Create multiple commits
    run_indra(&["init"], db_str);
    run_indra(&["create", "First", "--id", "first"], db_str);
    run_indra(&["create", "Second", "--id", "second"], db_str);
    run_indra(&["create", "Third", "--id", "third"], db_str);

    // Verify commit history
    let (stdout, _stderr, success) = run_indra(&["log"], db_str);

    assert!(success, "log should succeed");
    assert!(
        stdout.contains("\"count\":3"),
        "should have 3 commits (one per create)"
    );
}

// ============================================================================
// Search Tests
// ============================================================================

#[test]
fn test_cli_search_basic() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "The cat sat on the mat", "--id", "cat"], db_str);
    run_indra(
        &["create", "Dogs love to play fetch", "--id", "dog"],
        db_str,
    );

    // Note: Without HF embeddings, search uses mock embedder which does keyword matching
    let (stdout, _stderr, success) = run_indra(&["search", "cat"], db_str);

    assert!(success, "search should succeed");
    assert!(stdout.contains("\"count\":"), "should return count");
}

// ============================================================================
// Status Tests
// ============================================================================

#[test]
fn test_cli_status() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "A thought"], db_str);

    let (stdout, _stderr, success) = run_indra(&["status"], db_str);

    assert!(success, "status should succeed");
    assert!(
        stdout.contains("\"branch\":\"main\""),
        "should be on main branch"
    );
    assert!(
        stdout.contains("\"dirty\":false"),
        "should not be dirty after auto-commit"
    );
}

// ============================================================================
// Branch Tests
// ============================================================================

#[test]
fn test_cli_branches() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);
    run_indra(&["create", "Initial thought"], db_str);

    // Create branch
    let (stdout, _stderr, success) = run_indra(&["branch", "feature"], db_str);
    assert!(success, "branch creation should succeed");

    // List branches
    let (stdout, _stderr, success) = run_indra(&["branches"], db_str);
    assert!(success, "branches list should succeed");
    assert!(stdout.contains("main"), "should have main branch");
    assert!(stdout.contains("feature"), "should have feature branch");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_cli_empty_content() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    // Empty content should still work
    let (stdout, _stderr, success) = run_indra(&["create", ""], db_str);
    assert!(success, "empty content should be allowed");
    assert!(stdout.contains("\"status\":\"ok\""));
}

#[test]
fn test_cli_special_characters_in_content() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    let special_content = r#"Special chars: "quotes", 'apostrophes', \backslash, emoji 🎉"#;
    let (stdout, _stderr, success) = run_indra(&["create", special_content], db_str);

    assert!(success, "special characters should be handled");
    assert!(stdout.contains("\"status\":\"ok\""));
}

#[test]
fn test_cli_unicode_content() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    let unicode_content = "Unicode: 日本語 中文 한국어 العربية";
    let (stdout, _stderr, success) =
        run_indra(&["create", unicode_content, "--id", "unicode"], db_str);

    assert!(success, "unicode should be handled");

    // Verify retrieval
    let (stdout, _, success) = run_indra(&["get", "unicode"], db_str);
    assert!(success);
    assert!(stdout.contains("日本語"), "unicode should be preserved");
}

#[test]
fn test_cli_very_long_content() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join(".indra");
    let db_str = db_path.to_str().unwrap();

    run_indra(&["init"], db_str);

    // Create a long string (10KB)
    let long_content = "x".repeat(10_000);
    let (stdout, _stderr, success) = run_indra(&["create", &long_content], db_str);

    assert!(success, "long content should be handled");
    assert!(stdout.contains("\"status\":\"ok\""));
}
