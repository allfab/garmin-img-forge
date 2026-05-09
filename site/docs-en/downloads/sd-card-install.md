---
title: SD Card Installation
---

# :material-micro-sd: SD Card Installation

Step-by-step guide to install a Garmin map downloaded from this site onto a micro-SD card.

---

## Prerequisites

!!! info "Micro-SD card"
    Use a blank or freshly formatted micro-SD card (min. 8 GB — max. 32 GB).

<figure markdown>
  ![Micro-SD card](../assets/images/sdcard/sdcard.png)
  <figcaption>Micro-SD card ready to use</figcaption>
</figure>

---

## Procedure

### 1. Download the map

Go to the [Downloads](france.md) page and download the desired map(s).

Files are in `.img` format, named according to the convention:

```
IGN-BDTOPO-D038-v2026.03.img     ← department (Isère)
IGN-BDTOPO-LA-REUNION-v2026.03.img  ← overseas territory
IGN-BDTOPO-FRANCE-SE-v2026.03.img   ← France quadrant
```

### 2. Rename the file

Garmin devices only accept files named `gmapsupp.img`, `gmapsupp1.img`, `gmapsupp2.img`, etc.

Rename the downloaded file to `gmapsupp.img` before copying it to the SD card.

### 3. Create the Garmin folder and copy the file

Create a folder named **`Garmin`** at the root of your micro-SD card (if it doesn't already exist), then copy the renamed file into it.

!!! warning "Exact location"
    The `gmapsupp.img` file must be located in `Garmin/` **at the root** of the SD card, not in a subfolder.

### 4. Insert the card into the device

Remove the micro-SD card from your reader and insert it into your Garmin device.

### 5. Activate the map

After these operations, verify that the map is active in your GPS: **Settings > Map > Map Information**.

---

## Installing multiple maps

It is possible to install multiple maps simultaneously on your Garmin device.

To do this:

1. Download the additional map.
2. Rename the downloaded `.img` file by suffixing it: `gmapsupp1.img`, `gmapsupp2.img`, etc., depending on the number of maps already present in the `Garmin/` folder of your SD card.
3. Copy this file to the `Garmin/` folder on your micro-SD card.

<figure markdown>
  ![Multiple maps on the SD card](../assets/images/sdcard/sdcard-files.png)
  <figcaption>Example of an SD card with multiple Garmin map files</figcaption>
</figure>

!!! warning "Mandatory naming"
    Garmin devices only recognize files named `gmapsupp.img`, `gmapsupp1.img`, `gmapsupp2.img`, etc. A file with any other name (e.g. `IGN-BDTOPO-D038-v2026.03.img`) will not be detected.
