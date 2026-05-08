# imgforge — Compile le fichier Garmin IMG

**imgforge** prend un dossier de fichiers **Polish Map** (`.mp`)
et les compile en un seul binaire **Garmin IMG** (`.img`),
prêt à être chargé sur n'importe quel GPS Garmin ou dans QMapShack.

## Commande

```
imgforge build <mp_dir> --output <gmapsupp.img>
```

Le fichier `.img` final embarque toutes les tuiles, la symbologie TYP et l'index RGN.
