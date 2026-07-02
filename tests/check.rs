use assert_cmd::Command;
use serde_json::Value;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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

/// Copies the fixture, runs `analyze` to build a fresh graph.db (the
/// fixture's hand-written manifest.json simulates a prior `sync`), and
/// returns the repo root ready for `check`.
fn prepare(fixture: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixture_dir(fixture), dir.path());

    Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .arg("analyze")
        .assert()
        .success();

    dir
}

#[test]
fn check_on_drifted_fixture_reports_seeded_findings_and_exits_1() {
    let dir = prepare("drifted");

    let output = Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .env_remove("OPENAI_API_KEY")
        .args(["check", "--format", "json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let findings: Value = serde_json::from_slice(&output).expect("stdout should be valid JSON");
    let findings = findings.as_array().expect("findings should be an array");

    let rules: std::collections::HashSet<&str> = findings
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();

    assert_eq!(
        rules,
        std::collections::HashSet::from([
            "missing-entity",
            "managed-block-edited",
            "gone-symbol",
            "removed-route",
            "unknown-dependency",
            "diagram-node-gone",
        ]),
        "expected every seeded drift rule to fire exactly once each (removed-route fires twice)"
    );
    assert_eq!(
        findings.len(),
        7,
        "expected 7 total findings (2 removed-route)"
    );
}

#[test]
fn check_on_clean_fixture_exits_0_with_no_findings() {
    let dir = prepare("clean");

    let output = Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .env_remove("OPENAI_API_KEY")
        .args(["check", "--format", "json"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();

    let findings: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(findings.as_array().unwrap().len(), 0);
}

#[test]
fn check_makes_no_network_calls_and_needs_no_api_key() {
    // If `check` ever accidentally constructed a synthesis client, this
    // would fail loudly (missing/invalid key) instead of silently passing.
    let dir = prepare("clean");

    Command::cargo_bin("livingdocs")
        .unwrap()
        .current_dir(dir.path())
        .env_remove("OPENAI_API_KEY")
        .arg("check")
        .assert()
        .success()
        .stdout(predicates::str::contains("no drift found"));
}
