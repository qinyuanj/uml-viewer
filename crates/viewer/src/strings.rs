//! The viewer's slice of the single Chinese UI string table
//! (`love/uml/application/strings.lua`). Terms follow GLOSSARY.md; metric
//! names and key names stay English.

pub const NO_DATA: &str = "无数据";
pub const BACK: &str = "< 返回";
pub const PROPOSAL_BANNER: &str = "提案（未在源码中实例化）";
/// F3 overlay; matches `性能：%d FPS · 帧耗时 %.2f ms`.
pub fn performance(fps: u32, frame_ms: f64) -> String {
    format!("性能：{fps} FPS · 帧耗时 {frame_ms:.2} ms")
}
/// Empty canvas while a `--backend herdr` launch waits for mail (#139; the
/// mailbox itself is #142).
pub const WAITING_DISPLAY: &str = "等待 :display 命令加载图示…";
/// The diagram file named on the command line does not exist.
pub fn missing_file(path: &str) -> String {
    format!("找不到 IR 文件：{path}")
}
