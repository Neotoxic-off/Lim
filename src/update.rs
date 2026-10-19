//! `:upgrade` / `<Leader>pu`: asks GitHub for the latest release and, when it is newer, downloads the binary built
//! for this platform and puts it in place of this one. HTTPS goes through ureq and rustls, in process.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use std::{env, iter};

use crate::config::json::{self, Json};

const LATEST: &str = "https://api.github.com/repos/Neotoxic-off/Lim/releases/latest";
const DOWNLOAD: &str = "https://github.com/Neotoxic-off/Lim/releases/download";
/// A release binary is a few MB; anything far bigger is not one.
const MAX_BODY: u64 = 64 * 1024 * 1024;

/// The body of `url`, over HTTPS only (redirects too), with the proxy from the environment or Windows' settings.
/// Any status but 200 is an error (`HTTP 404`).
fn https_get(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .http_status_as_error(false)
        .user_agent("lim")
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut response: ureq::http::Response<ureq::Body> = agent.get(url).call().map_err(|e| e.to_string())?;
    let status: u16 = response.status().as_u16();
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    response.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(|e| e.to_string())
}

/// Runs on a background thread while editing goes on; what it returns becomes a notification.
pub fn run() -> Result<String, String> {
    let exe: PathBuf = env::current_exe().map_err(|e| format!("cannot find lim's own file: {e}"))?;
    upgrade(env!("CARGO_PKG_VERSION"), &exe)
}

/// Puts the latest release in place of `exe`, a lim of version `current`, when the release is newer.
fn upgrade(current: &str, exe: &PathBuf) -> Result<String, String> {
    let reply: Vec<u8> = https_get(LATEST).map_err(|e| {
        if e.contains("404") {
            "no release published on GitHub yet".into()
        } else {
            format!("cannot reach GitHub ({e})")
        }
    })?;
    let tag: String = latest_tag(&String::from_utf8_lossy(&reply)).ok_or("GitHub's answer has no release tag")?;
    if !newer(&tag, current) {
        return Ok(format!("lim {current} is up to date (latest release: {tag})"));
    }
    let asset: String = asset();
    let binary: Vec<u8> = https_get(&format!("{DOWNLOAD}/{tag}/{asset}")).map_err(|e| {
        if e.contains("404") {
            format!("{tag} has no {asset}; reinstall with the installer")
        } else {
            format!("download: {e}")
        }
    })?;
    let new: PathBuf = exe.with_extension("new");
    let written: io::Result<()> = fs::write(&new, &binary)
        .and_then(|()| fs::metadata(exe))
        .and_then(|m| fs::set_permissions(&new, m.permissions()));
    if let Err(e) = written {
        let _: io::Result<()> = fs::remove_file(&new);
        return Err(format!("cannot write {}: {e}", new.display()));
    }
    // The one process: the new binary must start here and say its version before it replaces this one.
    let expected: String = format!("lim {}", tag.trim_start_matches('v'));
    let says: io::Result<Output> = Command::new(&new).arg("--version").stdin(Stdio::null()).output();
    if !says.as_ref().is_ok_and(|out| String::from_utf8_lossy(&out.stdout).trim() == expected) {
        let _: io::Result<()> = fs::remove_file(&new);
        return Err(format!("the {tag} binary does not run on this system; lim {current} kept"));
    }
    replace(exe, &new).map_err(|e| format!("cannot replace {}: {e}", exe.display()))?;
    Ok(format!("lim updated to {tag}: restart lim to use it"))
}

/// The release asset for this platform, as the release workflow names it: `lim-x86_64-linux`, `lim-x86_64-windows.exe`.
fn asset() -> String {
    format!("lim-{}-{}{}", env::consts::ARCH, env::consts::OS, env::consts::EXE_SUFFIX)
}

/// Puts `new` in place of the running `exe`. Windows cannot overwrite a running exe but can rename it.
fn replace(exe: &PathBuf, new: &PathBuf) -> io::Result<()> {
    if !cfg!(windows) {
        return fs::rename(new, exe);
    }
    let old: PathBuf = exe.with_extension("old");
    let _: io::Result<()> = fs::remove_file(&old);
    fs::rename(exe, &old)?;
    fs::rename(new, exe).inspect_err(|_| {
        let _: io::Result<()> = fs::rename(&old, exe);
    })
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
    use std::path::Path;

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

    #[test]
    fn names_the_asset_of_this_platform() {
        let name: String = asset();
        assert!(name.starts_with("lim-") && name.contains(env::consts::OS), "{name}");
        assert_eq!(Path::new(&name).extension().is_some_and(|e| e == "exe"), cfg!(windows));
    }

    #[test]
    #[ignore = "network: cargo test https -- --ignored"]
    fn https_reaches_github() {
        let reply: Vec<u8> = https_get("https://api.github.com/repos/cli/cli/releases/latest").unwrap();
        let tag: String = latest_tag(&String::from_utf8_lossy(&reply)).unwrap();
        // A release asset redirects to another host, as lim's own binaries do.
        let url: String = format!("https://github.com/cli/cli/releases/download/{tag}/gh_{}_checksums.txt", &tag[1..]);
        assert!(String::from_utf8_lossy(&https_get(&url).unwrap()).contains("linux_amd64"));
        assert_eq!(https_get("https://api.github.com/repos/cli/cli/releases/tags/no-such-tag"), Err("HTTP 404".into()));
    }

    #[test]
    #[ignore = "network: cargo test upgrades -- --ignored"]
    fn upgrades_an_old_lim_to_the_latest_release() {
        let dir: PathBuf = env::temp_dir().join(format!("lim-upgrade-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe: PathBuf = dir.join(format!("lim{}", env::consts::EXE_SUFFIX));
        fs::write(&exe, "an old lim").unwrap();
        // The new binary takes the old one's permissions, and an installed lim is executable.
        #[cfg(unix)]
        fs::set_permissions(&exe, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let done: String = upgrade("0.0.1", &exe).unwrap();
        let said: Output = Command::new(&exe).arg("--version").output().unwrap();
        let version: String = String::from_utf8_lossy(&said.stdout).trim().to_owned();
        assert!(done.contains(version.trim_start_matches("lim ")), "{done} / {version}");
        // Now at the latest release: nothing to do.
        let current: &str = version.trim_start_matches("lim ");
        assert!(upgrade(current, &exe).unwrap().contains("up to date"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn replaces_a_file_in_place() {
        let dir: PathBuf = env::temp_dir().join(format!("lim-replace-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let (exe, new): (PathBuf, PathBuf) = (dir.join("lim"), dir.join("lim.new"));
        fs::write(&exe, "old").unwrap();
        fs::write(&new, "new").unwrap();
        replace(&exe, &new).unwrap();
        assert_eq!(fs::read_to_string(&exe).unwrap(), "new");
        assert!(!new.exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
