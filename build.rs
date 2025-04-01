use std::process::Command;

fn commit_info_from_git() -> Option<String> {
    Command::new("git")
        .args([
            "log",
            "-n 1",
            "--date=short",
            "--format= (%h %cd)",
            "--abbrev=8",
            "HEAD",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
}

fn main() {
    println!(
        "cargo:rustc-env=VERSION_INFO={}{}",
        env!("CARGO_PKG_VERSION"),
        commit_info_from_git().unwrap_or_default()
    );
}
