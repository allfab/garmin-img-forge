use std::env;
use std::process::Command;

fn main() {
    // Essayer de récupérer la version depuis Git
    let git_version = get_git_version();

    // Injecter comme variable d'environnement de compilation
    println!("cargo:rustc-env=GIT_VERSION={}", git_version);

    // Rerun si .git/HEAD, refs/tags ou packed-refs changent (les tags
    // peuvent être stockés loose dans refs/tags/ ou packés après git gc).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/tags");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");

    // Rerun si les sources du crate changent — sinon Cargo désactive le scan
    // par défaut dès qu'on émet au moins un rerun-if-changed, et GIT_VERSION
    // reste figée quand on modifie un fichier sans committer.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    // Rerun si les variables CI changent
    println!("cargo:rerun-if-env-changed=CI_COMMIT_TAG");
    println!("cargo:rerun-if-env-changed=CI_COMMIT_SHA");
}

fn get_git_version() -> String {
    // Priorité 1: Variables d'environnement CI (Woodpecker, GitHub Actions, GitLab CI)
    if let Some(version) = try_ci_tag() {
        return version;
    }

    // Priorité 2: git describe --tags (développement local)
    if let Some(version) = try_git_describe() {
        return version;
    }

    // Priorité 3: git rev-parse (juste le commit hash)
    if let Some(hash) = try_git_hash() {
        return hash;
    }

    // Fallback final: utiliser la version du Cargo.toml
    env!("CARGO_PKG_VERSION").to_string()
}

fn try_ci_tag() -> Option<String> {
    // Woodpecker CI / GitLab CI (même variable)
    if let Ok(tag) = env::var("CI_COMMIT_TAG") {
        if !tag.is_empty() {
            // "mpforge-v1.0.0" → "v1.0.0" ; "imgforge-v1.0.0" → "v1.0.0"
            let clean = strip_known_prefixes(&tag).to_string();
            return Some(clean);
        }
    }

    // GitHub Actions
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
        .map(|s| {
            let trimmed = s.trim();
            // "mpforge-v1.0.0-3-gabcdef" ou "imgforge-v1.0.0-3-gabcdef" → "v1.0.0-3-gabcdef"
            strip_known_prefixes(trimmed).to_string()
        })
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
    const PREFIXES: &[&str] = &["mpforge-", "imgforge-"];
    for prefix in PREFIXES {
        if let Some(stripped) = tag.strip_prefix(prefix) {
            return stripped;
        }
    }
    tag
}
