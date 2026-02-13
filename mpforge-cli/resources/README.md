# Ressources embarquées

Ce répertoire contient les fichiers qui sont embarqués dans le binaire au moment de la compilation.

## proj.db

**Ce fichier n'est PAS versionné dans Git.**

Il est automatiquement copié ici pendant le build CI depuis `/opt/proj-static/share/proj/proj.db`.

Le fichier est ensuite embarqué dans le binaire via `include_bytes!()` dans `src/proj_init.rs`,
ce qui permet au binaire d'être 100% autonome sans dépendance externe.

## Pour les développeurs

Si vous voulez compiler localement avec support PROJ complet, vous devez :

1. Installer PROJ (version 9.3.1 ou supérieure)
2. Copier manuellement proj.db ici :
   ```bash
   cp /usr/share/proj/proj.db resources/
   ```

Si `proj.db` n'est pas présent, la compilation échouera avec :
```
error: couldn't read ..../resources/proj.db: No such file or directory
```

## Alternative : Build sans proj.db embarqué

Pour compiler sans embarquer proj.db (mode développement), vous pouvez :

1. Créer un fichier vide `resources/proj.db` : `touch resources/proj.db`
2. Définir `PROJ_DATA` manuellement avant d'exécuter le binaire :
   ```bash
   export PROJ_DATA=/usr/share/proj
   ./target/debug/mpforge-cli --version
   ```
