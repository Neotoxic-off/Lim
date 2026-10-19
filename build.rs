//! The version lim reports (`LIM_VERSION`). A release's Cargo.toml carries it: the workflow writes it on the tag.
//! On master Cargo.toml stays at x.y.0, so a build from a clone numbers itself as the workflow would, x.y.<commits>,
//! and `<Leader>pu` does not offer the release it already is.

use std::env;
use std::process::{Command, Output};

fn main() {
    let version: String = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let from_git: Option<String> = version.strip_suffix(".0").and_then(|base| {
        let out: Output = Command::new("git").args(["rev-list", "--count", "HEAD"]).output().ok()?;
        let commits: String = String::from_utf8(out.stdout).ok()?.trim().to_owned();
        (out.status.success() && !commits.is_empty()).then(|| format!("{base}.{commits}"))
    });
    println!("cargo:rustc-env=LIM_VERSION={}", from_git.unwrap_or(version));
    // A commit or a checkout appends to the reflog.
    println!("cargo:rerun-if-changed=.git/logs/HEAD");
}
