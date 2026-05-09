# `--base-id` Convention by Coverage Type

The `--base-id` flag of `scripts/build-garmin-map.sh` sets the numeric Garmin identifier of the compiled map (first of the 8 digits of the `.img` sub-file name in `gmapsupp.img`). It must be **unique per coverage** within the same Garmin family to avoid collisions when installing multiple maps on the same GPS simultaneously.

!!! info "Automatic resolution"
    If `--base-id` is not provided, the script attempts automatic deduction from the first zone (e.g. `D038` → `38`, cf. `scripts/build-garmin-map.sh:580-584`). **For regions, quadrants and national coverages, no deduction is made** — you must pass `--base-id` explicitly or follow the convention below.

## Existing INSEE code ranges

| Range | Usage |
|-------|-------|
| `01`–`95`, `2A`, `2B` | Metropolitan departments |
| `971`–`978` | DOM + Saint-Barthélemy / Saint-Martin |
| `984` | TAAF |
| `986`–`988` | Wallis-et-Futuna, French Polynesia, New Caledonia |
| `R11`, `R24`, `R27`, `R28`, `R32`, `R44`, `R52`, `R53`, `R75`, `R76`, `R84`, `R93`, `R94` | Regions (INSEE regional codes) |

No INSEE code exists for **France quadrants** (`FRANCE-NO/NE/SO/SE`) or for half-France or all of France — these are **project-specific** subdivisions.

## Project convention (proposed)

The `base-id` values are chosen to **avoid any collision with INSEE codes** (notably DOM-COM in `971–988`). The convention has three tiers:

### Departments (tier 1–978)

→ Direct INSEE code (already automatically deduced by the script).

**Metropolitan France:**

| Zone | `--base-id` |
|------|-------------|
| D001 (Ain) | `1` |
| D038 (Isère) | `38` |
| D075 (Paris) | `75` |
| D02A (Corse-du-Sud) | `201` (INSEE statistical code) |
| D02B (Haute-Corse) | `202` (INSEE statistical code) |
| D095 (Val-d'Oise) | `95` |

!!! warning "Special case: Corsica"
    Departments `2A` and `2B` (Corse-du-Sud and Haute-Corse) share the numeric prefix `2`, which would cause a **Garmin collision** if both maps were installed simultaneously on the same GPS. The script therefore applies the **INSEE statistical mapping** `2A → 201` / `2B → 202` (codes used by INSEE in numeric datasets), guaranteeing their uniqueness.

**Overseas (DOM + COM):**

| Zone | INSEE code | `--base-id` |
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
| French Polynesia | 987 | `987` |
| New Caledonia | 988 | `988` |

### Regions (tier 111–194)

→ **`100 + R-INSEE code`**: offset of 100 to exit the department range.

| Region | INSEE code | `--base-id` |
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

### Quadrants & national (tier 910–999)

→ Range `910–999`, **strictly above regional codes (`≤ 194`) and below DOM-COM (`≥ 971`)** for quadrants. The national code `999` remains safe as no INSEE territory reaches this value.

| Coverage | `--base-id` | Description |
|------------|-------------|-------------|
| `FRANCE-NO` | `910` | North-West quadrant |
| `FRANCE-NE` | `920` | North-East quadrant |
| `FRANCE-SO` | `930` | South-West quadrant |
| `FRANCE-SE` | `940` | South-East quadrant |
| `FRANCE-NORD` | `950` | Northern half (FRANCE-NO + FRANCE-NE) |
| `FRANCE-SUD` | `960` | Southern half (FRANCE-SO + FRANCE-SE) |
| `FXX` | `999` | Metropolitan France (complete) |

!!! warning "Potential conflict with DOM-COM"
    Values `971–988` are **reserved for overseas INSEE codes**. Never use a `--base-id` in this range for a project coverage; it is exclusively allocated to the potential publication of DOM-COM maps (department by department).

### Defensive variant (optional)

To guard against a hypothetical future extension of COM INSEE codes, some projects place non-INSEE coverages in a `9xxx` range (4 digits), completely disjoint:

| Coverage | `--base-id` variant |
|------------|---------------------|
| `FRANCE-NO / NE / SO / SE` | `9001 / 9002 / 9003 / 9004` |
| `FRANCE-NORD / SUD` | `9010 / 9020` |
| `FXX` | `9999` |

This variant is more defensive but breaks the visual 3-digit consistency of other codes. **The project retains the 910–999 convention**, sufficient in practice.

## Invocation examples

```bash
# Department (base-id auto-deduced from D038)
./scripts/build-garmin-map.sh --zones D038 --year 2026 --version v2026.03

# Auvergne-Rhône-Alpes region
./scripts/build-garmin-map.sh --region ARA --base-id 184 --year 2026 --version v2026.03

# South-East France quadrant
./scripts/build-garmin-map.sh --region FRANCE-SE --base-id 940 --year 2026 --version v2026.03

# Complete metropolitan France
./scripts/build-garmin-map.sh --region FXX --base-id 999 --year 2026 --version v2026.03
```

## Installing multiple maps on the same GPS

The constraint is simple: **two simultaneously installed maps must not share the same `base-id`**. The convention above guarantees that a user can coexist on their GPS:

- An Isère map (`base-id=38`)
- An Auvergne-Rhône-Alpes map (`base-id=184`)
- A FRANCE-SE map (`base-id=940`)
- A DOM Guadeloupe map (`base-id=971`)

…without any identifier conflicts.
