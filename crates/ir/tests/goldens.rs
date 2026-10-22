//! Golden parity (#136, Seam A): for each fixture project the `ir` output is
//! structurally equal to the frozen JVM golden, and byte-identical across
//! repeated runs. Comparison parses both sides with the same EDN codec, so
//! golden pretty-printing and key order do not matter.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn generate(fixture: &str, output: &Path) -> std::process::ExitCode {
    let root = repo().join(fixture);
    let args: Vec<OsString> = [
        "--root",
        root.to_str().unwrap(),
        "--policy",
        "policy.edn",
        "--output",
        output.to_str().unwrap(),
    ]
    .iter()
    .map(OsString::from)
    .collect();
    ir::run(&args)
}

fn parsed(path: &Path) -> edn::Value {
    edn::parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn lua_fixture_matches_the_jvm_golden() {
    let output = std::env::temp_dir().join("uml-ir-test-lua.edn");
    assert_eq!(
        generate("lua-fixture", &output),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        parsed(&output),
        parsed(&repo().join("fixtures/jvm-goldens/lua.ir.edn"))
    );
}

#[test]
fn rust_fixture_matches_the_jvm_golden() {
    let output = std::env::temp_dir().join("uml-ir-test-rust.edn");
    assert_eq!(
        generate("rust-fixture", &output),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        parsed(&output),
        parsed(&repo().join("fixtures/jvm-goldens/rust.ir.edn"))
    );
}

#[test]
fn typescript_fixture_matches_the_jvm_golden() {
    let output = std::env::temp_dir().join("uml-ir-test-typescript.edn");
    assert_eq!(
        generate("typescript-fixture", &output),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        parsed(&output),
        parsed(&repo().join("fixtures/jvm-goldens/typescript.ir.edn"))
    );
}

#[test]
fn python_fixture_matches_the_jvm_golden() {
    let output = std::env::temp_dir().join("uml-ir-test-python.edn");
    assert_eq!(
        generate("python-fixture", &output),
        std::process::ExitCode::SUCCESS
    );
    assert_eq!(
        parsed(&output),
        parsed(&repo().join("fixtures/jvm-goldens/python.ir.edn"))
    );
}

#[test]
fn identical_input_produces_byte_identical_output() {
    let output = std::env::temp_dir().join("uml-ir-test-stable.edn");
    let mut runs = std::collections::HashSet::new();
    for _ in 0..4 {
        assert_eq!(
            generate("lua-fixture", &output),
            std::process::ExitCode::SUCCESS
        );
        runs.insert(std::fs::read(&output).unwrap());
    }
    assert_eq!(1, runs.len(), "IR bytes changed between identical runs");
}

#[test]
fn lua_namespaces_match_the_crapper_and_mutator_naming() {
    // The quality overlay joins snapshots by the class :ns, and crapper and
    // mutator name a module by its project-relative path. For the fixture
    // (:src "src" :prefix "calc") the scanned namespaces must equal the
    // snapshot namespaces exactly.
    let output = std::env::temp_dir().join("uml-ir-test-lua-ns.edn");
    assert_eq!(
        generate("lua-fixture", &output),
        std::process::ExitCode::SUCCESS
    );
    let document = parsed(&output);
    let namespaces: Vec<&str> = document
        .get(&edn::Value::keyword("classes"))
        .and_then(|c| match c {
            edn::Value::Vector(items) => Some(items),
            _ => None,
        })
        .unwrap()
        .iter()
        .filter_map(|class| match class.get(&edn::Value::keyword("ns")) {
            Some(edn::Value::String(ns)) => Some(ns.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(vec!["calc", "calc.util"], namespaces);
    let crap = parsed(&repo().join("lua-fixture/.metrics/crap.edn"));
    let snapshot_namespaces: std::collections::HashSet<&str> = crap
        .get(&edn::Value::keyword("entries"))
        .and_then(|e| match e {
            edn::Value::Vector(items) => Some(items),
            _ => None,
        })
        .unwrap()
        .iter()
        .filter_map(|entry| match entry.get(&edn::Value::keyword("namespace")) {
            Some(edn::Value::String(ns)) => Some(ns.as_str()),
            _ => None,
        })
        .collect();
    for ns in &namespaces {
        assert!(
            snapshot_namespaces.contains(ns),
            "class :ns {ns} missing from the crapper snapshot"
        );
    }
}
