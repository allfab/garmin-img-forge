# Versioning des binaires `imgforge` et `mpforge`

Les deux outils Rust du projet — [`imgforge`](../le-projet/imgforge.md) (compilateur Garmin IMG) et [`mpforge`](../le-projet/mpforge.md) (tuileur Polish Map) — embarquent à la compilation une chaîne de version calculée depuis l'état Git du dépôt. Cette page documente **comment lire la sortie de `--version`**, **d'où elle vient**, et **comment produire une release propre**.

!!! note "Attention aux deux systèmes de versioning"
    Le versioning décrit ici concerne uniquement **les binaires** (outils). Les cartes `.img` publiées dans la section [Téléchargements](../telechargements/index.md) suivent un schéma distinct (`v2026.03` = millésime BDTOPO, indépendant de la version du compilateur qui les a produites).

---

## TL;DR — lire la sortie de `--version`

Les deux outils exposent la version via le flag `--version` :

```bash
imgforge --version   # imgforge v0.4.3-49-geedfa8d-dirty
mpforge --version    # mpforge v0.4.2-49-geedfa8d
```

La chaîne après le nom de l'outil est produite par `git describe --tags --always --dirty`, après dépouillement du préfixe `imgforge-` ou `mpforge-` :

| Sortie observée | Signification |
|-----------------|---------------|
| `v0.4.3` | Build réalisé exactement sur le tag `imgforge-v0.4.3` — release propre. |
| `v0.4.3-49-geedfa8d` | 49 commits après le dernier tag, sur le commit `eedfa8d`. |
| `v0.4.3-49-geedfa8d-dirty` | Idem, mais avec des modifs trackées non committées dans l'arbre de travail. |
| `eedfa8d` | Aucun tag atteignable — fallback `--always` sur le hash court. |
| `0.4.3` | Pas de Git disponible au build — fallback sur la version du `Cargo.toml` du crate. |

!!! tip "Lecture rapide"
    - Un suffixe `-N-g<hash>` **sans** `-dirty` → binaire **source-traçable** : on peut retrouver le commit exact qui l'a produit.
    - Un suffixe `-dirty` → le binaire inclut des modifications locales non committées ; sa provenance ne peut plus être attestée.

Pour une simple vérification de provenance, la lecture s'arrête ici. Les sections suivantes intéressent les mainteneurs et contributeurs.

---

## Comment la version est résolue (mainteneur)

La chaîne est injectée dans le binaire via la variable d'environnement de compilation `GIT_VERSION`, calculée par le script de build de chaque crate (`tools/imgforge/build.rs`, `tools/mpforge/build.rs`).

### Ordre de priorité

Le premier candidat valide gagne :

**Sources CI (variables d'environnement)**

| # | Variable | Pipeline concerné |
|---|----------|-------------------|
| 1 | `CI_COMMIT_TAG` | Woodpecker — `.woodpecker/{imgforge,mpforge}.yml` sur push de tag |

**Sources Git (commandes locales)**

| # | Commande | Utilisé quand |
|---|----------|---------------|
| 2 | `git describe --tags --always --dirty` | Développement local |
| 3 | `git rev-parse --short HEAD` | Fallback théorique — inatteignable en pratique, `git describe --always` couvrant déjà ce cas |

**Fallback Rust**

| # | Source | Utilisé quand |
|---|--------|---------------|
| 4 | `env!("CARGO_PKG_VERSION")` | Aucune source Git accessible (ex. build depuis un tarball sans `.git/`) |

La valeur retenue est publiée via `cargo:rustc-env=GIT_VERSION=…` puis lue côté runtime par `env!("GIT_VERSION")`, branchée sur clap.

!!! info "Équivalence CI sur tag et build local sur tag"
    En CI, `CI_COMMIT_TAG` gagne (priorité 1) car Woodpecker clone souvent en `--depth=1` où `git describe` ne verrait pas les tags. En local sur un tag checkouté (`git checkout imgforge-v0.4.3`), `git describe` (priorité 2) produit la même chaîne après dépouillement. Le résultat est identique, les chemins sont différents.

!!! warning "Variables CI réellement supportées"
    Le code lit aussi `GITHUB_REF` pour GitHub Actions et émet `rerun-if-env-changed=CI_COMMIT_SHA`, mais **aucun pipeline GitHub Actions n'existe** dans ce dépôt à ce jour. `CI_COMMIT_SHA` est vestigial et n'est actuellement lu par aucun chemin de code. À nettoyer lors d'une prochaine passe sur `build.rs`.

---

## Convention de tags Git

Le dépôt héberge **deux crates** (`imgforge`, `mpforge`) dans un monorepo. Pour que chacun puisse avoir son cycle de release sans collision, les tags sont préfixés :

| Outil | Format de tag | Exemple |
|-------|---------------|---------|
| `imgforge` | `imgforge-v<X.Y.Z>` | `imgforge-v0.4.3` |
| `mpforge`  | `mpforge-v<X.Y.Z>`  | `mpforge-v0.4.2` |

À la compilation, `build.rs` dépouille ces préfixes avant de générer la chaîne `GIT_VERSION` : un tag `imgforge-v0.4.3` produit `v0.4.3` dans le binaire, pas `imgforge-v0.4.3`.

!!! danger "Piège local : les tags des deux outils ne sont pas isolés"
    L'appel `git describe --tags` **sans `--match`** prend le tag le plus récent atteignable dans l'historique, quel que soit son préfixe. En développement local, `mpforge --version` peut donc afficher `v0.4.3-N-g<hash>` (tag imgforge) alors que son `Cargo.toml` porte `0.4.2`. Ce n'est pas un bug d'affichage, c'est une limite actuelle de `build.rs` : une fix propre consiste à appeler `git describe --tags --match '<outil>-v*'` dans chaque crate.

---

## Le watcher Cargo (piège subtil)

Dès qu'un `build.rs` émet au moins une directive `cargo:rerun-if-changed=...`, Cargo **désactive son scan par défaut** du répertoire du package. Sans watcher explicite sur les sources, `build.rs` ne ré-exécute pas quand un fichier `.rs` est modifié, et `GIT_VERSION` reste figée à sa valeur du dernier build qui a réellement re-tourné `git describe`.

Les deux `build.rs` déclarent donc les watchers suivants :

| Chemin surveillé | Ce que ça ré-exécute `build.rs` pour |
|------------------|--------------------------------------|
| `../../.git/HEAD` | Changement de commit (checkout, commit, reset) |
| `../../.git/refs/tags` | Création de tags loose (`git tag X`) |
| `../../.git/packed-refs` | Tags packés après `git gc` |
| `src` | Modification d'une source du crate |
| `Cargo.toml` | Bump de version, changement de dépendance |
| `Cargo.lock` | `cargo update` (même sans toucher `Cargo.toml`) |

!!! warning "Ne pas retirer ces directives"
    Retirer `rerun-if-changed=src` fige `GIT_VERSION` à la valeur du dernier build qui a réellement ré-exécuté `build.rs`. Le suffixe `-dirty` n'apparaît **pas** à cause de la directive `src` — il provient de `git describe --dirty` exécuté par `build.rs`. La directive `src` se contente de garantir que `build.rs` re-tourne quand les sources changent, donc que `git describe` est ré-interrogé et que l'affichage reflète l'état réel du working tree.

---

## Quand `-dirty` apparaît

Le suffixe provient de `git describe --dirty`, qui regarde uniquement les fichiers **déjà suivis par Git**.

| État du working tree | Suffixe `-dirty` |
|----------------------|------------------|
| Modifier un fichier tracké dans `src/` | Oui |
| Staging (`git add`) sans commit | Oui |
| Créer un **nouveau** fichier non tracké dans `src/` | Non |
| Modifier `Cargo.lock` | Oui (si tracké) |

!!! note "Limite connue"
    Un fichier ajouté mais jamais `git add`-é passe sous le radar de `git describe --dirty`. C'est le comportement standard de Git.

---

## Workflow de release (mainteneur)

Pour publier une nouvelle version d'un des deux outils :

### 1. Bump de version

```bash
# Exemple : imgforge 0.4.3 → 0.4.4
vim tools/imgforge/Cargo.toml   # version = "0.4.4"
cd tools/imgforge && cargo build --release   # régénère tools/imgforge/Cargo.lock
```

Chaque crate a son propre `Cargo.lock` local (pas de Cargo workspace unifié).

### 2. Synchroniser les références documentaires

Plusieurs pages du site contiennent des URLs `wget` versionnées pointant vers la release Forgejo correspondante. À mettre à jour **en même temps** que le bump :

| Fichier | Référence versionnée |
|---------|----------------------|
| `site/docs/le-projet/imgforge.md` | Section *Installation* → `wget .../imgforge-v<X.Y.Z>/...` |
| `site/docs/le-projet/mpforge.md` | Section *Installation* → `wget .../mpforge-v<X.Y.Z>/...` |
| `site/docs/prerequis-installation/procedure-installation.md` | URLs `wget` d'installation |

### 3. Commit + tag

```bash
git add tools/imgforge/Cargo.toml tools/imgforge/Cargo.lock site/docs/...
git commit -m "release(imgforge): v0.4.4"
git tag imgforge-v0.4.4
```

Le push (commit + tag) est fait ensuite par le mainteneur selon son workflow habituel.

### 4. Build et publication du binaire (CI)

Sur arrivée du tag, `.woodpecker/imgforge.yml` (ou `mpforge.yml`) :

- détecte `CI_COMMIT_TAG=imgforge-v0.4.4`, `build.rs` dépouille le préfixe, `GIT_VERSION=v0.4.4` injecté ;
- génère un `CHANGELOG` automatique à partir du range `PREVIOUS_TAG..CI_COMMIT_TAG` ;
- publie l'archive `imgforge-linux-amd64.tar.gz` sur la release Forgejo du tag.

### 5. Sanity check

```bash
./target/release/imgforge --version
# Doit afficher exactement : imgforge v0.4.4
```

Tout autre affichage (suffixe `-N-g<hash>`, `-dirty`, ou hash nu) indique que le build n'a **pas** été réalisé sur le tag exact.

---

## Cohérence carte ↔ outil ↔ tag

Le projet publie deux catégories d'artefacts à la fois :

| Artefact | Versioning | Exemple |
|----------|------------|---------|
| Binaire outil (`imgforge`, `mpforge`) | SemVer préfixé `{outil}-v<X.Y.Z>` | `imgforge-v0.4.3` |
| Carte `.img` publiée (section [Téléchargements](../telechargements/index.md)) | Millésime annuel `v<YYYY>.<MM>` | `v2026.03` |

Les deux systèmes sont **disjoints** : la version du compilateur utilisé pour produire une carte n'est pas inscrite dans le nom du fichier `.img` (elle l'est dans `manifest.json` sous la clé `build_params` des coverages).

Pour attester la provenance d'un binaire installé, `{outil} --version` et le tag Forgejo correspondant sont le seul couple fiable. Si un doute subsiste sur la carte elle-même, consulter le `manifest.json` publié à côté du fichier `.img`.

---

*Page connexe :* [`imgforge` — le compilateur](../le-projet/imgforge.md) · [`mpforge` — le tuileur](../le-projet/mpforge.md)
