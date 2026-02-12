use std::process::Command;

fn main() {
    // Essayer de récupérer la version depuis Git
    let git_version = get_git_version();

    // Injecter comme variable d'environnement de compilation
    println!("cargo:rustc-env=GIT_VERSION={}", git_version);

    // Rerun si .git/HEAD ou refs/tags changent
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/tags");
}

fn get_git_version() -> String {
    // Essayer git describe --tags
    if let Some(version) = try_git_describe() {
        return version;
    }

    // Fallback: essayer git rev-parse (juste le commit hash)
    if let Some(hash) = try_git_hash() {
        return hash;
    }

    // Fallback final: utiliser la version du Cargo.toml
    env!("CARGO_PKG_VERSION").to_string()
}

fn try_git_describe() -> Option<String> {
    let output = Command::new("git")
        .args(&["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

fn try_git_hash() -> Option<String> {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}
