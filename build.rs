use std::process::Command;

struct CommitInfo {
    hash: String,
    short_hash: String,
    date: String,
}

fn commit_info_from_git() -> Option<CommitInfo> {
    let output = Command::new("git")
        .args([
            "log",
            "-1",
            "--date=short",
            "--format=%H %h %cd",
            "--abbrev=9",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())?;

    let mut parts = output.split_whitespace().map(|s| s.to_string());

    Some(CommitInfo {
        hash: parts.next()?,
        short_hash: parts.next()?,
        date: parts.next()?,
    })
}

fn main() {
    let version = commit_info_from_git()
        .map(|info| format!(" ({} {} {})", info.hash, info.short_hash, info.date))
        .unwrap_or_default();

    println!(
        "cargo:rustc-env=VERSION_INFO={}{version}",
        env!("CARGO_PKG_VERSION")
    );
}
