# Convention `--base-id` par type de couverture

Le flag `--base-id` de `scripts/build-garmin-map.sh` fixe l'identifiant numérique Garmin de la carte compilée (premier des 8 chiffres du nom de sous-fichier `.img` dans le `gmapsupp.img`). Il doit être **unique par couverture** au sein d'une même famille Garmin pour éviter les collisions lors de l'installation simultanée de plusieurs cartes sur un même GPS.

!!! info "Résolution automatique"
    Si `--base-id` n'est pas fourni, le script tente une déduction automatique depuis la première zone (ex: `D038` → `38`, cf. `scripts/build-garmin-map.sh:580-584`). **Pour les régions, quadrants et couvertures nationales, aucune déduction n'est faite** — il faut passer `--base-id` explicitement ou suivre la convention ci-dessous.

## Rappel des codes INSEE existants

| Plage | Usage |
|-------|-------|
| `01`–`95`, `2A`, `2B` | Départements métropolitains |
| `971`–`978` | DOM + Saint-Barthélemy / Saint-Martin |
| `984` | TAAF |
| `986`–`988` | Wallis-et-Futuna, Polynésie française, Nouvelle-Calédonie |
| `R11`, `R24`, `R27`, `R28`, `R32`, `R44`, `R52`, `R53`, `R75`, `R76`, `R84`, `R93`, `R94` | Régions (codes INSEE régionaux) |

Aucun code INSEE n'existe pour les **quadrants France** (`FRANCE-NO/NE/SO/SE`) ni pour les demi-France ou la France entière — ce sont des découpages **spécifiques au projet**.

## Convention projet (proposée)

Les `base-id` sont choisis pour **éviter toute collision avec les codes INSEE** (notamment les DOM-COM en `971–988`). La convention s'articule en trois paliers :

### Départements (palier 1–978)

→ Code INSEE direct (déjà déduit automatiquement par le script).

**Métropole :**

| Zone | `--base-id` |
|------|-------------|
| D001 (Ain) | `1` |
| D038 (Isère) | `38` |
| D075 (Paris) | `75` |
| D02A (Corse-du-Sud) | `2` (simplifié par le script) |
| D02B (Haute-Corse) | `2` (simplifié par le script) |
| D095 (Val-d'Oise) | `95` |

**Outre-mer (DOM + COM) :**

| Zone | Code INSEE | `--base-id` |
|------|-----------|-------------|
| Guadeloupe | 971 | `971` |
| Martinique | 972 | `972` |
| Guyane | 973 | `973` |
| La Réunion | 974 | `974` |
| Saint-Pierre-et-Miquelon | 975 | `975` |
| Mayotte | 976 | `976` |
| Saint-Barthélemy | 977 | `977` |
| Saint-Martin | 978 | `978` |
| Terres australes et antarctiques françaises (TAAF) | 984 | `984` |
| Wallis-et-Futuna | 986 | `986` |
| Polynésie française | 987 | `987` |
| Nouvelle-Calédonie | 988 | `988` |

### Régions (palier 111–194)

→ **`100 + code R-INSEE`** : décalage de 100 pour sortir de la plage des départements.

| Région | Code INSEE | `--base-id` |
|--------|-----------|-------------|
| Île-de-France (IDF) | R11 | `111` |
| Centre-Val de Loire (CVL) | R24 | `124` |
| Bourgogne-Franche-Comté (BFC) | R27 | `127` |
| Normandie (NOR) | R28 | `128` |
| Hauts-de-France (HDF) | R32 | `132` |
| Grand Est (GES) | R44 | `144` |
| Pays de la Loire (PDL) | R52 | `152` |
| Bretagne (BRE) | R53 | `153` |
| Nouvelle-Aquitaine (NAQ) | R75 | `175` |
| Occitanie (OCC) | R76 | `176` |
| Auvergne-Rhône-Alpes (ARA) | R84 | `184` |
| Provence-Alpes-Côte d'Azur (PAC) | R93 | `193` |
| Corse (COR) | R94 | `194` |

### Quadrants & national (palier 910–999)

→ Plage `910–999`, **strictement au-dessus des codes régionaux (`≤ 194`) et en dessous des DOM-COM (`≥ 971`)** pour les quadrants. Le code national `999` reste safe car aucun territoire INSEE n'atteint cette valeur.

| Couverture | `--base-id` | Description |
|------------|-------------|-------------|
| `FRANCE-NO` | `910` | Quart Nord-Ouest |
| `FRANCE-NE` | `920` | Quart Nord-Est |
| `FRANCE-SO` | `930` | Quart Sud-Ouest |
| `FRANCE-SE` | `940` | Quart Sud-Est |
| `FRANCE-NORD` | `950` | Moitié Nord (FRANCE-NO + FRANCE-NE) |
| `FRANCE-SUD` | `960` | Moitié Sud (FRANCE-SO + FRANCE-SE) |
| `FXX` | `999` | France métropolitaine entière |

!!! warning "Conflit potentiel avec les DOM-COM"
    Les valeurs `971–988` sont **réservées aux codes INSEE Outre-mer**. N'utilisez jamais un `--base-id` dans cette plage pour une couverture projet ; elle est exclusivement allouée à la publication éventuelle de cartes DOM-COM (département par département).

### Variante défensive (optionnelle)

Pour se prémunir contre une hypothétique extension future des codes INSEE COM, certains projets placent les couvertures non-INSEE dans une plage `9xxx` (4 chiffres), totalement disjointe :

| Couverture | `--base-id` variante |
|------------|---------------------|
| `FRANCE-NO / NE / SO / SE` | `9001 / 9002 / 9003 / 9004` |
| `FRANCE-NORD / SUD` | `9010 / 9020` |
| `FXX` | `9999` |

Cette variante est plus défensive mais casse la cohérence visuelle 3-chiffres des autres codes. **Le projet retient la convention 910–999**, suffisante en pratique.

## Exemples d'invocation

```bash
# Département (base-id auto-déduit depuis D038)
./scripts/build-garmin-map.sh --zones D038 --year 2026 --version v2026.03

# Région Auvergne-Rhône-Alpes
./scripts/build-garmin-map.sh --region ARA --base-id 184 --year 2026 --version v2026.03

# Quadrant France Sud-Est
./scripts/build-garmin-map.sh --region FRANCE-SE --base-id 940 --year 2026 --version v2026.03

# France métropolitaine complète
./scripts/build-garmin-map.sh --region FXX --base-id 999 --year 2026 --version v2026.03
```

## Installer plusieurs cartes sur un même GPS

La contrainte est simple : **deux cartes installées simultanément ne doivent pas partager le même `base-id`**. La convention ci-dessus garantit qu'un utilisateur peut cohabiter sur son GPS :

- Une carte Isère (`base-id=38`)
- Une carte Auvergne-Rhône-Alpes (`base-id=184`)
- Une carte FRANCE-SE (`base-id=940`)
- Une carte DOM Guadeloupe (`base-id=971`)

…sans aucun conflit d'identifiants.
