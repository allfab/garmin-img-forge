# Déploiement du site documentation via nginx

Ce dossier contient la configuration pour servir le site Zensical sous `https://maps.garmin.allfabox.fr/` via un conteneur nginx statique.

## Architecture

```
Woodpecker CI                    Serveur (forge LXC)
┌──────────────┐                ┌───────────────────────────────┐
│ push site/** │                │  Traefik (websecure)          │
│      │       │                │       │                       │
│  build site  │                │  nginx:alpine :8081           │
│      │       │                │       │                       │
│  cp → bind   │──────────────> │  ./sites/garmin-ign-topo-map/ │
│    mount     │                │  www/                         │
└──────────────┘                └───────────────────────────────┘
```

## Prérequis

- Docker et Docker Compose installés sur le serveur
- Réseau Docker `forge` existant (utilisé par Forgejo)
- Traefik configuré avec config dynamique YAML et certResolver `letsencrypt`
- DNS : `maps.garmin.allfabox.fr` pointant vers l'IP du serveur Traefik (A record Cloudflare)
- **Woodpecker runner** : configurer `WOODPECKER_BACKEND_DOCKER_VOLUMES` pour autoriser le chemin hôte `/opt/docker/forge-stack/sites/garmin-ign-topo-map/www`

## Procédure de déploiement

### 1. Créer le répertoire du site

```bash
mkdir -p /opt/docker/forge-stack/sites/garmin-ign-topo-map/www/
```

### 2. Démarrer nginx

```bash
cd /opt/docker/forge-stack/sites/garmin-ign-topo-map/
docker compose up -d
```

Le conteneur `garmin-ign-topo-map` expose le port 8081 sur l'hôte et sert les fichiers depuis `./sites/garmin-ign-topo-map/www/`.

> **Note :** Au premier démarrage, le répertoire est vide. nginx renverra une erreur 403 jusqu'au premier déploiement via le pipeline CI (étape 4).

### 3. Configurer Traefik

Copier le fichier de config dynamique dans le répertoire surveillé par Traefik :

```bash
cp traefik-site-docs.yml <TRAEFIK_DYNAMIC_DIR>/
```

Traefik détecte automatiquement le nouveau fichier et configure le routage vers `http://10.10.20.51:8081`.

### 4. Vérifier le DNS

```bash
dig maps.garmin.allfabox.fr +short
```

### 5. Déclencher le premier déploiement

Pousser un changement dans `site/` sur la branche `main`, ou lancer manuellement le pipeline `site.yml` depuis Woodpecker CI.

## Vérification

```bash
# Vérifier que nginx est démarré et healthy
docker ps --filter name=garmin-ign-topo-map

# Vérifier que le site répond
curl -I https://maps.garmin.allfabox.fr/
# Attendu : HTTP 200, Server: nginx
```

## Fichiers

| Fichier | Description |
|---------|-------------|
| `docker-compose.yml` | Service nginx `garmin-ign-topo-map`, port 8081, bind mount, réseau `forge`, healthcheck |
| `traefik-site-docs.yml` | Config dynamique Traefik (router + service vers nginx) |

## Notes

- Aucun token API Forgejo n'est nécessaire (nginx sert des fichiers statiques)
- Le secret Woodpecker `pages_token` peut être supprimé
- La branche `pages` du repo peut être supprimée (nettoyage)
