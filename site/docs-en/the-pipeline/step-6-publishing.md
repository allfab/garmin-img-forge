# Step 6 — Publishing

Once the `gmapsupp.img` is compiled (step 4), two publishing targets are possible via the `--publish` option of `scripts/build-garmin-map.sh`:

| Target | Variable | Destination | Usage |
|---|---|---|---|
| **local** *(default)* | `PUBLISH_TARGET=local` | `site/docs/telechargements/files/` | Direct integration into the MkDocs site |
| **s3** | `PUBLISH_TARGET=s3` | Garage S3 bucket via `rclone` | Externalized hosting (large files) |

In both cases, the script updates `site/docs/telechargements/manifest.json` (catalog of published versions) and patches the relevant Markdown pages (download links + SHA256).

---

## `local` target

```bash
./scripts/build-garmin-map.sh --region ARA --publish
```

Default behavior if `PUBLISH_TARGET` is not defined. Copies the `.img` directly into `site/docs/telechargements/files/<coverage>/<version>/`.

---

## Publishing without rebuilding {#publishing-without-rebuilding}

When a map has already been validated (GPS tests, QmapShack inspection...) and you only want to republish it or push it to another target, simply relaunch the script with `--skip-existing` and the desired target:

```bash
./scripts/build-garmin-map.sh \
    --region FRANCE-SE \
    --version v2026.03 \
    ... \
    --skip-existing \
    --publish \
    --publish-target s3
```

Thanks to `--skip-existing`:

- **Phase 1 mpforge**: existing `.mp` files are preserved (a quick scan of extents, ~1-4 min).
- **Phase 2 imgforge**: if the target `.img` already exists in `pipeline/output/<...>/img/`, **the rebuild is entirely skipped** (no file deletion, no recompilation).
- **Publishing**: runs normally with the already present `.img`.

This is particularly useful for:

- Switching from a `local` publish to `s3` (or vice versa) after validation.
- Re-triggering `update_manifest` and the MkDocs page patch after an editorial modification.
- Pushing the same build to multiple S3 buckets (by varying `PUBLISH_TARGET` / `S3_BUCKET`).

!!! warning "Parameter consistency"
    `--skip-existing` only checks for the presence of the target file, not its build parameters.
    If you have changed `--family-name`, `--base-id`, geometry options, or the YAML config
    since the last build, **delete the `.img` and the relevant `.mp` files** to force a rebuild.

---

## `s3` target (Garage)

### `.env` configuration

```dotenv
PUBLISH_TARGET=s3

# rclone remote "garage:" via env vars (no rclone.conf)
RCLONE_CONFIG_GARAGE_TYPE=s3
RCLONE_CONFIG_GARAGE_PROVIDER=Other
RCLONE_CONFIG_GARAGE_ACCESS_KEY_ID=<access-key>
RCLONE_CONFIG_GARAGE_SECRET_ACCESS_KEY=<secret-key>
RCLONE_CONFIG_GARAGE_ENDPOINT=https://garage-api.example.com
RCLONE_CONFIG_GARAGE_REGION=garage
RCLONE_CONFIG_GARAGE_ACL=public-read

S3_BUCKET=<bucket-name>
PUBLIC_URL_BASE=https://download-maps.example.com
```

The 5 required variables (`RCLONE_CONFIG_GARAGE_ACCESS_KEY_ID`, `_SECRET_ACCESS_KEY`, `_ENDPOINT`, `S3_BUCKET`, `PUBLIC_URL_BASE`) are validated at script startup; absence = error before build.

### Infrastructure prerequisites

The Garage server exposes **two distinct endpoints**:

| Port | Role | Exposed to |
|---|---|---|
| **3900** | S3 API (AWS v4 signed) | Authenticated uploads via `rclone` |
| **3902** | Website (anonymous HTTP read) | End clients downloading `.img` files |

The bucket must have **Website Access** enabled and an alias consistent with `root_domain`:

```bash
garage bucket website --allow <bucket-name> --index-document index.html
garage bucket alias <bucket-id> <bucket-name>
```

### Reverse proxy (Caddy example)

```caddy
# S3 API — upload
garage-api.example.com {
    reverse_proxy http://<garage-host>:3900
}

# Website — public download
download-maps.example.com {
    reverse_proxy http://<garage-host>:3902 {
        header_up Host "<bucket-name>.<root_domain>"
    }
}
```

The rewritten `Host` is crucial: Garage routes by virtual-host `<bucket>.<root_domain>` as defined in `garage.toml` (`[s3_web] root_domain`).

### Firewall

Port **3902** must be open between the reverse proxy and Garage (in addition to 3900 for the API). On Proxmox LXC, edit `/etc/pve/firewall/<VMID>.fw`:

```
[RULES]
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3900 -log nolog # GARAGE S3 API
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3902 -log nolog # GARAGE S3 Website
IN ACCEPT -source +dc/<ipset> -p tcp -dport 3909 -log nolog # GARAGE Dashboard
```

### Connection test

Before the first `--publish`, validate the complete chain with:

```bash
./scripts/test-s3-connection.sh
```

5 sequential tests: env vars, `rclone lsd garage:`, bucket listing, upload/read/SHA256/delete round-trip, and HTTP public request to `PUBLIC_URL_BASE`. The `-v` flag activates `rclone -vv` for signed header dump (useful for diagnosing 403 errors).

Expected HTTP code on `PUBLIC_URL_BASE/`: **404** (empty bucket, no `index.html`) → this is a functional success, it proves that Caddy and Garage website are communicating.

---

## Retention

Script `scripts/prune-s3.sh` — deletes old versions from the bucket while keeping the N most recent per coverage:

```bash
./scripts/prune-s3.sh --dry-run             # simulation (display all)
./scripts/prune-s3.sh --keep 3              # keep 3 versions/coverage
./scripts/prune-s3.sh --coverage departement/d038 --keep 2
```

Updates `manifest.json` accordingly (git commit remains manual).

---

## Manifest

The file `site/docs/telechargements/manifest.json` centralizes all published versions (across all targets). It is consumed on the site side by `site/docs/javascripts/downloads-manifest.js` to display available maps and their checksums.

Structure:

```json
{
  "coverages": {
    "departement/d038": {
      "latest": "v2025.12",
      "latest_url": "https://download-maps.example.com/departement/d038/v2025.12/IGN-BDTOPO-D038-v2025.12.img",
      "versions": [
        { "version": "v2025.12", "path": "...", "sha256": "...", "size": 123456 }
      ]
    }
  }
}
```
