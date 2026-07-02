use assert_cmd::Command;
use serde_json::Value;

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/basic-ts")
}

#[test]
fn analyze_dry_run_prints_extracted_symbols_as_json() {
    let output = Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(fixture_dir())
        .args(["analyze", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let files: Value = serde_json::from_slice(&output).expect("stdout should be valid JSON");
    let files = files.as_array().expect("top level JSON should be an array");
    assert_eq!(files.len(), 3, "expected all three fixture files parsed");

    let by_file = |name: &str| -> &Value {
        files
            .iter()
            .find(|f| f["file"].as_str().unwrap().ends_with(name))
            .unwrap_or_else(|| panic!("missing entry for {name}"))
    };

    let user_service = by_file("user-service.ts");
    assert_eq!(user_service["classes"][0]["name"], "UserService");
    assert_eq!(
        user_service["classes"][0]["methods"],
        serde_json::json!(["constructor", "create", "delete"])
    );
    assert_eq!(user_service["interfaces"][0]["name"], "User");
    assert_eq!(user_service["imports"][0]["source"], "./policy-service");

    let policy_service = by_file("policy-service.ts");
    assert_eq!(policy_service["functions"][0]["name"], "calculatePremium");
    assert_eq!(policy_service["classes"][0]["name"], "PolicyService");

    let index_js = by_file("index.js");
    assert_eq!(index_js["functions"][0]["name"], "bootstrap");
}

#[test]
fn analyze_populates_graph_db() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir(), dir.path());

    Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .arg("analyze")
        .assert()
        .success();

    let graph_path = dir.path().join(".livingdocs/graph.db");
    assert!(
        graph_path.exists(),
        "analyze should write .livingdocs/graph.db"
    );

    let conn = rusqlite::Connection::open(&graph_path).unwrap();
    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap();
    assert_eq!(file_count, 3);

    let symbol_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
        .unwrap();
    assert!(symbol_count > 0, "expected symbols to be recorded");

    let dependency_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM dependencies WHERE from_file = 'src/user-service.ts' AND to_file = 'src/policy-service.ts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(dependency_count, 1);
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let dest_path = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&dest_path).unwrap();
            copy_dir(&entry.path(), &dest_path);
        } else {
            std::fs::copy(entry.path(), &dest_path).unwrap();
        }
    }
}

#[test]
fn analyze_without_config_fails_with_helpful_message() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .args(["analyze", "--dry-run"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("livingdocs init"));
}
