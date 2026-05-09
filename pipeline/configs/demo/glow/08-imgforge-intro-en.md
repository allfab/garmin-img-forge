# imgforge — Compiles the Garmin IMG file

**imgforge** takes a folder of **Polish Map** files (`.mp`)
and compiles them into a single **Garmin IMG** (`.img`) binary,
ready to be loaded on any Garmin GPS or in QMapShack.

## Command

```
imgforge build <mp_dir> --output <gmapsupp.img>
```

The final `.img` file embeds all tiles, TYP symbology and the RGN index.
