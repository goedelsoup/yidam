use std::path::Path;

pub fn genesis_date(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["log", "--reverse", "--format=%as", "--max-count=1"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "no commits".to_string())
}

pub fn head_commit_short(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn genesis_message(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["log", "--reverse", "--format=%B", "--max-count=1"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn active_phase_count(root: &Path) -> usize {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["branch", "--list", "ma/*"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}
