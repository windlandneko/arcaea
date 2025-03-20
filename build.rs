use std::process::Command;

fn commit_info_from_git() -> Option<String> {
    Command::new("git")
        .args([
            "log",
            "-1",
            "--date=short",
            "--format= (%h %cd)",
            "--abbrev=8",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
}

fn main() {
    println!(
        "cargo:rustc-env=VERSION_INFO={}{}",
        env!("CARGO_PKG_VERSION"),
        commit_info_from_git().unwrap_or_default()
    );
}
