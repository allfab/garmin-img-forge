# Ressources embarquées

Ce répertoire contient les fichiers qui sont embarqués dans le binaire au moment de la compilation.

## proj.db

Le fichier `proj.db` est la base de données PROJ (bibliothèque de transformations de coordonnées).
Il est **versionné dans Git** pour garantir que la compilation fonctionne directement après un clone,
sans dépendance à une installation PROJ locale.

Le fichier est embarqué dans le binaire via `include_bytes!()` dans `src/proj_init.rs`,
ce qui permet au binaire d'être 100% autonome sans dépendance externe.

### Mise à jour de proj.db

Si vous devez mettre à jour vers une version plus récente de PROJ :

```bash
cp /usr/share/proj/proj.db resources/
```

Puis commiter le fichier mis à jour.
