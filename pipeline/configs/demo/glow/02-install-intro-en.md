# Tools Installation

The **garmin-img-forge** pipeline relies on two autonomous Rust binaries,
with no system dependency (no JVM, no Python, no separate GDAL).

## mpforge — Forges Polish Map tiles

Reads SHP/GPKG layers, splits according to the tiling grid,
applies Garmin rules and geometric simplification,
writes **Polish Map** files (`.mp`) — one per tile.

## imgforge — Compiles the Garmin IMG file

Takes a folder of `.mp` files and compiles them into a single
**Garmin IMG** (`.img`) binary, ready for any Garmin GPS or QMapShack.

## Installation

Download from GitHub releases, extract and copy to `~/.local/bin/`.
Same procedure for `mpforge` and `imgforge`.
