use std::env;
use std::process::Command;

fn main() {
    let git_version = get_git_version();
    println!("cargo:rustc-env=GIT_VERSION={}", git_version);
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-env-changed=CI_COMMIT_TAG");
    println!("cargo:rerun-if-env-changed=CI_COMMIT_SHA");
}

fn get_git_version() -> String {
    if let Some(version) = try_ci_tag() {
        return version;
    }
    if let Some(version) = try_git_describe() {
        return version;
    }
    if let Some(hash) = try_git_hash() {
        return hash;
    }
    env!("CARGO_PKG_VERSION").to_string()
}

fn try_ci_tag() -> Option<String> {
    if let Ok(tag) = env::var("CI_COMMIT_TAG") {
        if !tag.is_empty() {
            let clean = strip_known_prefixes(&tag).to_string();
            return Some(clean);
        }
    }
    if let Ok(ref_name) = env::var("GITHUB_REF") {
        if ref_name.starts_with("refs/tags/") {
            let tag = ref_name.trim_start_matches("refs/tags/");
            let clean = strip_known_prefixes(tag).to_string();
            return Some(clean);
        }
    }
    None
}

fn try_git_describe() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| strip_known_prefixes(s.trim()).to_string())
}

fn try_git_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

fn strip_known_prefixes(tag: &str) -> &str {
    const PREFIXES: &[&str] = &["imgforge-", "mpforge-"];
    for prefix in PREFIXES {
        if let Some(stripped) = tag.strip_prefix(prefix) {
            return stripped;
        }
    }
    tag
}
