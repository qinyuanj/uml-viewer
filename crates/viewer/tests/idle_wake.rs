//! Idle wake-ups (#143): an unchanged poll requests no repaint; a changed
//! diagram, metrics snapshot or mailbox does.

use std::path::PathBuf;
use viewer::watch::Wake;

fn project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("viewer-wake-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".metrics/mutate")).unwrap();
    std::fs::write(dir.join("diagram.edn"), "{:packages []}").unwrap();
    dir
}

#[test]
fn an_unchanged_poll_requests_no_repaint() {
    let dir = project("same");
    let mut wake = Wake::new(&dir.join("diagram.edn"), &dir);
    assert!(!wake.poll());
    assert!(!wake.poll());
}

#[test]
fn a_changed_diagram_requests_one_repaint() {
    let dir = project("ir");
    let mut wake = Wake::new(&dir.join("diagram.edn"), &dir);
    std::fs::write(dir.join("diagram.edn"), "{:packages [{:label \"P\"}]}").unwrap();
    assert!(wake.poll());
    assert!(!wake.poll());
}

#[test]
fn new_metrics_and_mail_request_a_repaint() {
    let dir = project("metrics");
    let mut wake = Wake::new(&dir.join("diagram.edn"), &dir);
    std::fs::write(dir.join(".metrics/mutate/a.edn"), "{}").unwrap();
    assert!(wake.poll());
    std::fs::create_dir_all(dir.join(".uml-viewer")).unwrap();
    std::fs::write(
        dir.join(".uml-viewer/to-viewer.edn"),
        "{:next-id 1 :queue []}",
    )
    .unwrap();
    assert!(wake.poll());
    assert!(!wake.poll());
}
