# Binary Versioning of `imgforge` and `mpforge`

The two Rust tools in the project — [`imgforge`](../the-project/imgforge.md) (Garmin IMG compiler) and [`mpforge`](../the-project/mpforge.md) (Polish Map tiler) — embed at compile time a version string calculated from the Git state of the repository. This page documents **how to read the `--version` output**, **where it comes from**, and **how to produce a clean release**.

!!! note "Two versioning systems"
    The versioning described here concerns only **binaries** (tools). The `.img` maps published in the [Downloads](../downloads/index.md) section follow a distinct scheme (`v2026.03` = BD TOPO vintage, independent of the compiler version that produced them).

---

## TL;DR — reading the `--version` output

Both tools expose the version via the `--version` flag:

```bash
imgforge --version   # imgforge v0.4.3-49-geedfa8d-dirty
mpforge --version    # mpforge v0.4.2-49-geedfa8d
```

The string after the tool name is produced by `git describe --tags --always --dirty`, after stripping the `imgforge-` or `mpforge-` prefix:

| Observed output | Meaning |
|-----------------|---------------|
| `v0.4.3` | Build done exactly on tag `imgforge-v0.4.3` — clean release. |
| `v0.4.3-49-geedfa8d` | 49 commits after the last tag, on commit `eedfa8d`. |
| `v0.4.3-49-geedfa8d-dirty` | Same, but with tracked uncommitted modifications in the working tree. |
| `eedfa8d` | No reachable tag — `--always` fallback on the short hash. |
| `0.4.3` | No Git available at build — fallback on the crate's `Cargo.toml` version. |

!!! tip "Quick read"
    - A `-N-g<hash>` suffix **without** `-dirty` → binary is **source-traceable**: the exact commit that produced it can be found.
    - A `-dirty` suffix → the binary includes uncommitted local modifications; its provenance can no longer be attested.

For a simple provenance check, reading stops here. The following sections are for maintainers and contributors.

---

## How the version is resolved (maintainer)

The string is injected into the binary via the `GIT_VERSION` compile-time environment variable, calculated by each crate's build script (`tools/imgforge/build.rs`, `tools/mpforge/build.rs`).

### Priority order

The first valid candidate wins:

**CI sources (environment variables)**

| # | Variable | Relevant pipeline |
|---|----------|-------------------|
| 1 | `CI_COMMIT_TAG` | Woodpecker — `.woodpecker/{imgforge,mpforge}.yml` on tag push |

**Git sources (local commands)**

| # | Command | Used when |
|---|----------|---------------|
| 2 | `git describe --tags --always --dirty` | Local development |
| 3 | `git rev-parse --short HEAD` | Theoretical fallback — unreachable in practice, `git describe --always` already covers this case |

**Rust fallback**

| # | Source | Used when |
|---|--------|---------------|
| 4 | `env!("CARGO_PKG_VERSION")` | No Git source accessible (e.g. build from tarball without `.git/`) |

The retained value is published via `cargo:rustc-env=GIT_VERSION=…` then read at runtime by `env!("GIT_VERSION")`, wired to clap.

!!! info "Equivalence between CI on tag and local build on tag"
    In CI, `CI_COMMIT_TAG` wins (priority 1) because Woodpecker often clones with `--depth=1` where `git describe` would not see tags. Locally on a checked out tag (`git checkout imgforge-v0.4.3`), `git describe` (priority 2) produces the same string after stripping. The result is identical, the paths are different.

!!! warning "Actually supported CI variables"
    The code also reads `GITHUB_REF` for GitHub Actions and emits `rerun-if-env-changed=CI_COMMIT_SHA`, but **no GitHub Actions pipeline exists** in this repository at this time. `CI_COMMIT_SHA` is vestigial and is currently read by no code path. To be cleaned up in a future `build.rs` pass.

---

## Git tag convention

The repository hosts **two crates** (`imgforge`, `mpforge`) in a monorepo. So that each can have its own release cycle without collision, tags are prefixed:

| Tool | Tag format | Example |
|-------|---------------|---------|
| `imgforge` | `imgforge-v<X.Y.Z>` | `imgforge-v0.4.3` |
| `mpforge`  | `mpforge-v<X.Y.Z>`  | `mpforge-v0.4.2` |

At compile time, `build.rs` strips these prefixes before generating the `GIT_VERSION` string: a tag `imgforge-v0.4.3` produces `v0.4.3` in the binary, not `imgforge-v0.4.3`.

!!! danger "Local trap: tags from the two tools are not isolated"
    The call `git describe --tags` **without `--match`** takes the most recently reachable tag in history, regardless of its prefix. In local development, `mpforge --version` may therefore display `v0.4.3-N-g<hash>` (imgforge tag) while its `Cargo.toml` carries `0.4.2`. This is not a display bug, it is a current limitation of `build.rs`: a proper fix consists of calling `git describe --tags --match '<tool>-v*'` in each crate.

---

## The Cargo watcher (subtle trap)

As soon as a `build.rs` emits at least one `cargo:rerun-if-changed=...` directive, Cargo **disables its default scan** of the package directory. Without an explicit watcher on sources, `build.rs` does not re-execute when a `.rs` file is modified, and `GIT_VERSION` stays frozen at its value from the last build that actually re-ran `git describe`.

Both `build.rs` files therefore declare the following watchers:

| Watched path | What it triggers `build.rs` re-execution for |
|------------------|--------------------------------------|
| `../../.git/HEAD` | Commit change (checkout, commit, reset) |
| `../../.git/refs/tags` | Loose tag creation (`git tag X`) |
| `../../.git/packed-refs` | Packed tags after `git gc` |
| `src` | Modification of a crate source |
| `Cargo.toml` | Version bump, dependency change |
| `Cargo.lock` | `cargo update` (even without touching `Cargo.toml`) |

!!! warning "Do not remove these directives"
    Removing `rerun-if-changed=src` freezes `GIT_VERSION` at the value from the last build that actually re-executed `build.rs`. The `-dirty` suffix does **not** appear because of the `src` directive — it comes from `git describe --dirty` executed by `build.rs`. The `src` directive simply guarantees that `build.rs` re-runs when sources change, so that `git describe` is re-queried and the display reflects the actual working tree state.

---

## When `-dirty` appears

The suffix comes from `git describe --dirty`, which only looks at files **already tracked by Git**.

| Working tree state | `-dirty` suffix |
|----------------------|-----------------|
| Modifying a tracked file in `src/` | Yes |
| Staging (`git add`) without commit | Yes |
| Creating a **new** untracked file in `src/` | No |
| Modifying `Cargo.lock` | Yes (if tracked) |

!!! note "Known limitation"
    A file added but never `git add`-ed flies under the radar of `git describe --dirty`. This is standard Git behavior.

---

## Release workflow (maintainer)

To publish a new version of one of the two tools:

### 1. Version bump

```bash
# Example: imgforge 0.4.3 → 0.4.4
vim tools/imgforge/Cargo.toml   # version = "0.4.4"
cd tools/imgforge && cargo build --release   # regenerates tools/imgforge/Cargo.lock
```

Each crate has its own local `Cargo.lock` (no unified Cargo workspace).

### 2. Synchronize documentation references

Several site pages contain versioned `wget` URLs pointing to the corresponding Forgejo release. Update **at the same time** as the bump:

| File | Versioned reference |
|---------|----------------------|
| `site/docs/le-projet/imgforge.md` | *Installation* section → `wget .../imgforge-v<X.Y.Z>/...` |
| `site/docs/le-projet/mpforge.md` | *Installation* section → `wget .../mpforge-v<X.Y.Z>/...` |
| `site/docs/prerequis-installation/procedure-installation.md` | Installation `wget` URLs |

### 3. Commit + tag

```bash
git add tools/imgforge/Cargo.toml tools/imgforge/Cargo.lock site/docs/...
git commit -m "release(imgforge): v0.4.4"
git tag imgforge-v0.4.4
```

The push (commit + tag) is then done by the maintainer according to their usual workflow.

### 4. Binary build and publication (CI)

On tag arrival, `.woodpecker/imgforge.yml` (or `mpforge.yml`):

- detects `CI_COMMIT_TAG=imgforge-v0.4.4`, `build.rs` strips the prefix, `GIT_VERSION=v0.4.4` injected;
- generates an automatic `CHANGELOG` from the range `PREVIOUS_TAG..CI_COMMIT_TAG`;
- publishes the `imgforge-linux-amd64.tar.gz` archive on the Forgejo release of the tag.

### 5. Sanity check

```bash
./target/release/imgforge --version
# Must display exactly: imgforge v0.4.4
```

Any other display (suffix `-N-g<hash>`, `-dirty`, or bare hash) indicates the build was **not** done on the exact tag.

---

## Map ↔ tool ↔ tag consistency

The project publishes two categories of artifacts simultaneously:

| Artifact | Versioning | Example |
|----------|------------|---------|
| Tool binary (`imgforge`, `mpforge`) | SemVer prefixed `{tool}-v<X.Y.Z>` | `imgforge-v0.4.3` |
| Published `.img` map ([Downloads](../downloads/index.md) section) | Annual vintage `v<YYYY>.<MM>` | `v2026.03` |

The two systems are **disjoint**: the compiler version used to produce a map is not inscribed in the `.img` filename (it is in `manifest.json` under the `build_params` key of coverages).

To attest the provenance of an installed binary, `{tool} --version` and the corresponding Forgejo tag are the only reliable pair. If there is doubt about the map itself, consult the `manifest.json` published alongside the `.img` file.

---

*Related pages:* [`imgforge` — the compiler](../the-project/imgforge.md) · [`mpforge` — the tiler](../the-project/mpforge.md)
