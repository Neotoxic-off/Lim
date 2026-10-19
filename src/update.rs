//! `:upgrade` / `<Leader>pu`: asks GitHub for the latest release and, when it is newer, rebuilds lim from that tag
//! with cargo, as the installer does. curl ships with Linux, macOS and Windows 10+, so lim needs no TLS of its own.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::{env, iter};

use crate::config::json::{self, Json};
use crate::editor::pipe;

const REPO: &str = "https://github.com/Neotoxic-off/Lim";
const LATEST: &str = "https://api.github.com/repos/Neotoxic-off/Lim/releases/latest";

/// Runs on a background thread while editing goes on; what it returns becomes a notification.
pub fn run() -> Result<String, String> {
    let current: &str = env!("CARGO_PKG_VERSION");
    let argv: Vec<String> = ["curl", "-fsSL", "-H", "User-Agent: lim", LATEST].map(str::to_owned).to_vec();
    let reply: String = pipe(&argv, "").map_err(|e| {
        if e.contains("404") {
            "no release published on GitHub yet".into()
        } else {
            format!("cannot reach GitHub ({e})")
        }
    })?;
    let tag: String = latest_tag(&reply).ok_or("GitHub's answer has no release tag")?;
    if !newer(&tag, current) {
        return Ok(format!("lim {current} is up to date (latest release: {tag})"));
    }
    // Windows cannot overwrite a running exe but can rename it, so this one steps aside for cargo.
    let exe: Option<PathBuf> = env::current_exe().ok().filter(|_| cfg!(windows));
    let old: Option<PathBuf> = exe.as_ref().map(|e| e.with_extension("old"));
    if let (Some(exe), Some(old)) = (&exe, &old) {
        let _: io::Result<()> = fs::remove_file(old);
        let _: io::Result<()> = fs::rename(exe, old);
    }
    // Captured, so cargo's progress never draws over the editor.
    let built: io::Result<Output> = Command::new("cargo")
        .args(["install", "--git", REPO, "--tag", &tag, "--locked", "--force", "lim"])
        .stdin(Stdio::null())
        .output();
    // Back in place unless cargo put the new one there (this exe was not the installed one, or the build failed).
    if let (Some(exe), Some(old)) = (&exe, &old)
        && !exe.exists()
    {
        let _: io::Result<()> = fs::rename(old, exe);
    }
    match built {
        Ok(out) if out.status.success() => Ok(format!("lim updated to {tag}: restart lim to use it")),
        Ok(out) => {
            let stderr: String = String::from_utf8_lossy(&out.stderr).into_owned();
            let error: &str = stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
            Err(format!("cargo {}: {}", out.status, error.trim()))
        }
        Err(e) => Err(format!("cargo: {e} (install Rust from https://rustup.rs)")),
    }
}

/// `tag_name` of GitHub's releases/latest answer.
fn latest_tag(reply: &str) -> Option<String> {
    let Ok(Json::Object(fields)): Result<Json, json::ParseError> = json::parse(reply) else { return None };
    fields.into_iter().find_map(|(k, v)| match v {
        Json::String(tag) if k == "tag_name" => Some(tag),
        _ => None,
    })
}

/// Whether release `tag` (`v1.2.3`) is a later version than `current` (`1.2.0`); a tag that is not numbers is not.
fn newer(tag: &str, current: &str) -> bool {
    let parts =
        |v: &str| -> Option<Vec<u64>> { v.trim_start_matches('v').split('.').map(|p| p.parse().ok()).collect() };
    let (Some(tag), Some(current)): (Option<Vec<u64>>, Option<Vec<u64>>) = (parts(tag), parts(current)) else {
        return false;
    };
    // `1.2` is `1.2.0`.
    let len: usize = tag.len().max(current.len());
    let pad = |v: Vec<u64>| -> Vec<u64> { v.iter().copied().chain(iter::repeat(0)).take(len).collect() };
    pad(tag) > pad(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_release_tag() {
        let reply: &str = r#"{ "url": "x", "tag_name": "v0.2.0", "assets": [{ "name": "lim" }], "draft": false }"#;
        assert_eq!(latest_tag(reply), Some("v0.2.0".into()));
        assert_eq!(latest_tag(r#"{ "message": "Not Found" }"#), None);
        assert_eq!(latest_tag(r#"{ "tag_name": 2 }"#), None);
        assert_eq!(latest_tag(r#"["v1.0.0"]"#), None);
        assert_eq!(latest_tag("<html>rate limited</html>"), None);
        assert_eq!(latest_tag(""), None);
    }

    #[test]
    fn compares_versions() {
        assert!(newer("v0.2.0", "0.1.0"));
        assert!(newer("v0.10.0", "0.9.9"));
        assert!(newer("1.0.1", "1.0"));
        assert!(!newer("v0.1.0", "0.1.0"));
        assert!(!newer("v1.0", "1.0.0"));
        assert!(!newer("v0.0.9", "0.1.0"));
        assert!(!newer("nightly", "0.1.0"));
        assert!(!newer("v2.0.0-rc1", "0.1.0"));
    }
}
