# Contribuer à `garmin-img-forge`

Merci de l'intérêt que vous portez au projet !

Ce dépôt GitHub est un **miroir public en lecture**. La source canonique est hébergée
sur Forgejo : [`forgejo.allfabox.fr/allfab/garmin-img-forge`](https://forgejo.allfabox.fr/allfab/garmin-img-forge).
Les développements actifs (commits, releases) se font côté Forgejo ; chaque push `main`
est répercuté ici par un job Woodpecker (`.woodpecker/mirror-github.yml`).

## Flux de contribution

### Issues

Les issues GitHub sont **ouvertes et bienvenues**. Utilisez les templates fournis
(`bug`, `enhancement`) pour un traitement plus rapide.

### Pull Requests GitHub

Les PR GitHub sont également acceptées, avec quelques particularités liées au
fonctionnement en miroir :

1. Vous ouvrez la PR classiquement côté GitHub.
2. Si elle est acceptée, les commits sont rapatriés et mergés par le mainteneur
   (`allfab`) **sur Forgejo**, puis le miroir répercute l'état vers GitHub.
3. Conséquence : votre PR GitHub passera automatiquement à l'état `closed` une fois
   que ses commits apparaîtront dans l'historique `main` côté miroir. C'est normal
   — votre contribution est bien intégrée, simplement pas via un merge GitHub natif.
4. Un label `upstream-forgejo` peut être appliqué pendant la phase de rapatriement
   pour tracer où en est votre PR.

### Délais

Le projet est un projet personnel maintenu sur temps libre. **Aucun délai de
traitement n'est garanti.** Un ping tous les 15 jours sur une issue/PR ouverte est
tolérable si vous n'avez pas eu de réponse.

### Contribution plus rapide

Si vous êtes motivé et que votre contribution est non triviale, il peut être plus
rapide de **créer un compte directement sur l'instance Forgejo** (ouverte aux
contributeurs) et de proposer votre PR à la source. Contactez `allfab` sur les
issues GitHub pour obtenir un accès.

## Périmètre des contributions

Ce qui est miroirisé et donc modifiable via PR :

- `tools/` (mpforge, imgforge, ogr-polishmap, ogr-garminimg)
- `pipeline/` **sauf** `data/` et `output/` (les configs, resources, scripts internes sont accessibles)
- `site/` (contenu et config Zensical)
- `scripts/`
- `.github/` (templates + workflow Pages)
- Fichiers racine (`README.md`, `LICENSE`, ce fichier, etc.)

Ce qui est **absent du miroir** (et donc inaccessible depuis GitHub) :

- `_bmad/` (workflows BMAD internes)
- `docs/` (planning-artifacts, implementation-artifacts, brainstorming)
- `pipeline/data/`, `pipeline/output/` (artefacts volumineux, générés)
- `.woodpecker/` (pipelines Woodpecker internes, non pertinents hors homelab)
- `.claude/`, `.vscode/`, `CLAUDE.md`, `.mcp.json` (configuration dev locale)
- Tous les fichiers `.env*` à tous les niveaux de l'arbre

Si votre PR nécessite des modifications dans un de ces chemins, ouvrez plutôt une
issue pour en discuter — le travail doit être fait côté Forgejo.

## Style

- **Shell** : `set -euo pipefail`, préférer POSIX `sh` sauf si bashisme nécessaire.
- **Rust** : suivre les conventions observées (`cargo fmt`, `cargo clippy` propre).
- **Python** : PEP 8 raisonnable, pas de linter imposé.
- **Commits** : messages en français ou anglais, préfixe `feat/fix/docs/chore/refactor`
  suivi du scope entre parenthèses (ex. `fix(imgforge): ...`).

## Licences

- `tools/ogr-polishmap/` : MIT
- `tools/mpforge/`, `tools/imgforge/` : GPL v3
- Documentation (site) : CC BY-SA 4.0

En contribuant, vous acceptez que votre contribution soit publiée sous la licence
du composant concerné.
