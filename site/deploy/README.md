# Déploiement du site documentation via Codeberg Pages Server

Ce dossier contient la configuration pour servir le site Zensical sous `https://maps.garmin.allfabox.fr/` via Codeberg Pages Server.

## Prérequis

- Docker et Docker Compose installés sur le serveur
- Réseau Docker `forge` existant (utilisé par Forgejo)
- Traefik configuré avec config dynamique YAML et certResolver `letsencrypt`
- **Traefik doit être connecté au réseau Docker `forge`** pour résoudre le nom `pages-server` (via `docker network connect forge traefik` ou en ajoutant le réseau dans le compose Traefik)
- DNS : `maps.garmin.allfabox.fr` pointant vers l'IP du serveur Traefik (A record Cloudflare)

## Procédure de déploiement

### 1. Créer les tokens API Forgejo

Deux tokens distincts sont nécessaires (principe du moindre privilège) :

**Token pages-server** (lecture seule, tourne en permanence dans le conteneur) :
1. Se connecter à Forgejo (`https://forgejo.allfabox.fr`)
2. **Settings > Applications > Generate New Token**
3. Nom : `pages-server-read`
4. Scope : `read:repository`
5. Copier le token

**Token CI deploy** (écriture, utilisé uniquement par le pipeline) :
1. **Settings > Applications > Generate New Token**
2. Nom : `woodpecker-pages-deploy`
3. Scope : `write:repository`
4. Copier le token

### 2. Configurer les variables d'environnement

Créer un fichier `.env` dans le répertoire du compose (`site/deploy/` ou là où le compose est déployé sur le serveur) :

```bash
PAGES_TOKEN=<token_pages-server-read>
```

### 3. Vérifier le DNS

S'assurer que `maps.garmin.allfabox.fr` pointe vers l'IP du serveur Traefik :

```bash
dig maps.garmin.allfabox.fr +short
```

### 4. Démarrer le pages-server

```bash
cd site/deploy/
docker compose up -d
```

Le pages-server n'expose aucun port sur l'hôte — il est accessible uniquement via le réseau Docker `forge` par Traefik.

### 5. Configurer Traefik

Copier le fichier de config dynamique dans le répertoire surveillé par Traefik :

```bash
cp traefik-pages-server.yml /chemin/vers/traefik/dynamic/
```

Traefik détecte automatiquement le nouveau fichier et configure le routage.

S'assurer que Traefik est connecté au réseau `forge` :

```bash
docker network connect forge traefik
```

### 6. Créer le secret Woodpecker CI

1. Aller dans les paramètres du dépôt dans Woodpecker CI
2. **Secrets > Add Secret**
3. Nom : `pages_token`
4. Valeur : le token `woodpecker-pages-deploy` (write) créé à l'étape 1
5. Events : `push`

### 7. Déclencher le premier déploiement

Deux options :
- Pousser un changement dans `site/` sur la branche `main`
- Lancer manuellement le pipeline `site.yml` depuis Woodpecker CI

## Vérification

```bash
# Vérifier que le pages-server est démarré
docker logs pages-server

# Vérifier que le site répond
curl -I https://maps.garmin.allfabox.fr/
```

## Architecture

```
Woodpecker CI                    Serveur
┌──────────────┐                ┌──────────────────────┐
│ push site/** │                │  Traefik (websecure) │
│      │       │                │       │              │
│  build site  │                │  pages-server:8080   │
│      │       │                │  (réseau forge only) │
│  push branch │───────────────>│  Forgejo API :3000   │
│   "pages"    │                │  (branche pages)     │
└──────────────┘                └──────────────────────┘
```

## Fichiers

| Fichier | Description |
|---------|-------------|
| `docker-compose.yml` | Service pages-server sur réseau `forge` (pas de port exposé) |
| `traefik-pages-server.yml` | Config dynamique Traefik (router + service) |
| `.env` | Token API Forgejo lecture seule (à créer manuellement, non versionné) |
