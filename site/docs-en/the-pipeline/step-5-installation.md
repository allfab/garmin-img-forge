# Step 5: GPS Installation

The last step is the simplest: copy the `gmapsupp.img` file onto the Garmin GPS.

---

## Procedure

### 1. Connect the GPS

Connect your Garmin GPS via USB to your computer, or insert the SD card into a reader.

The GPS (or the SD card) appears as a mass storage device.

### 2. Copy the file

```bash
# Identify the GPS mount point
lsblk
# or
mount | grep -i garmin

# Copy the file
cp output/gmapsupp.img /media/$USER/GARMIN/Garmin/

# Or to the SD card
cp output/gmapsupp.img /media/$USER/SD_CARD/Garmin/
```

!!! info "File location"
    The `gmapsupp.img` file must be placed in the `Garmin/` folder at the root of the GPS or SD card. This is the standard name automatically recognized by all Garmin GPS devices.

### 3. Restart the GPS

Safely eject the device, then restart the GPS. The map automatically appears in the map management menu.

## Compatible devices

These maps are compatible with all Garmin GPS devices supporting additional maps:

| Category | Models |
|----------|--------|
| **Outdoor watches** | fenix, Enduro, Instinct (certain models) |
| **Hiking GPS** | Oregon, eTrex, Montana, GPSMAP |
| **Cycling GPS** | Edge (certain models) |
| **Dog tracking** | Alpha 100F/200F/300F/50F, Astro 320 |

## Managing multiple maps

If you already have a `gmapsupp.img` file on your GPS (for example an OSM map), rename one of the two to avoid conflicts:

```bash
# Rename the existing map
mv /media/$USER/GARMIN/Garmin/gmapsupp.img /media/$USER/GARMIN/Garmin/gmapsupp_osm.img

# Copy the new map
cp output/gmapsupp.img /media/$USER/GARMIN/Garmin/gmapsupp_bdtopo.img
```

Garmin GPS devices automatically recognize all `.img` files in the `Garmin/` folder, regardless of their name.

## Enabling/disabling the map

On the GPS, go to **Settings > Map > Map Information** to enable or disable installed maps. This allows switching between multiple maps without deleting them.

## Verification on the GPS

Once the map is loaded, verify:

- **Roads** display correctly (zoom in/out)
- **POIs** are clickable and display their name
- **Polygons** (forests, lakes, buildings) are filled with the correct colors
- **Routing** works (if enabled): calculate a route between two points
- **Relief** (hill shading) is visible if DEM data was integrated
