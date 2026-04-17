# Déploiement du site documentation via nginx (LXC Apps)

Ce dossier contient la configuration pour servir le site Zensical sous `https://maps.garmin.allfabox.fr/` via un conteneur nginx statique hébergé dans le LXC **Apps** sur Proxmox Scaleway.

## Architecture

```
Woodpecker CI runner            Bastion SSH             LXC Apps (10.0.100.30)              Traefik Frontend
┌──────────────────┐         ┌──────────────┐          ┌────────────────────────────────┐   ┌──────────────┐
│ push site/**     │         │  allfab@     │          │  nginx:alpine :8880            │   │  websecure   │
│       │          │  rsync  │  163.172.    │  proxy   │       │                        │   │      │       │
│  build zensical  │────────>│  82.220      │─────────>│  /opt/docker/apps-stack/       │<──│  03-garmin   │
│       │          │  -J     │              │  jump    │     garmin-img-forge/www/ │   │   .yml       │
└──────────────────┘         └──────────────┘          └────────────────────────────────┘   └──────────────┘
                                 (ProxyJump)          user SSH: deploy (clé CI dédiée)
```

Le runner Woodpecker n'ayant pas d'accès direct au réseau privé du LXC Apps, il passe par un bastion SSH (`allfab@163.172.82.220`) en utilisant l'option `-J` (ProxyJump) de OpenSSH. La **même clé privée CI** authentifie les deux hops.

## Prérequis LXC Apps

- LXC **unprivileged** avec `features: nesting=1,keyctl=1` dans `/etc/pve/lxc/<vmid>.conf` (sans quoi Docker échoue sur cgroups).
- Docker Engine ≥ 24 + plugin Compose installés (`apt install docker-ce docker-ce-cli containerd.io docker-compose-plugin`).
- `rsync` installé (`apt install rsync`).
- DNS : `maps.garmin.allfabox.fr` pointant vers l'IP Traefik Frontend (déjà en place).
- Stack Traefik Frontend existante avec reload auto des fichiers dynamiques et `certResolver: letsencrypt`.

## Provisioning initial (manuel)

### 1. Réseau Docker

```bash
docker network create apps
```

### 2. Arborescence de déploiement

```bash
sudo mkdir -p /opt/docker/apps-stack/garmin-img-forge/www
```

### 3. User SSH `deploy` dédié au CI

```bash
sudo useradd -m -s /bin/bash deploy
sudo chown -R deploy:deploy /opt/docker/apps-stack/garmin-img-forge/www
```

Si `sshd_config` contient `AllowUsers`, ajouter `deploy` :

```bash
sudo sed -i 's/^AllowUsers allfab$/AllowUsers allfab deploy/' /etc/ssh/sshd_config
sudo sshd -t && sudo systemctl reload ssh
```

### 4. Déposer la clé publique CI dans `~deploy/.ssh/authorized_keys`

Depuis ton poste :

```bash
scp -o ProxyJump=allfab@163.172.82.220 \
  ~/.ssh/woodpecker-ci-agent.pub \
  allfab@10.0.100.30:/tmp/ci.pub
ssh -J allfab@163.172.82.220 allfab@10.0.100.30 \
  'sudo install -d -m 700 -o deploy -g deploy /home/deploy/.ssh && \
   sudo install -m 600 -o deploy -g deploy /tmp/ci.pub /home/deploy/.ssh/authorized_keys && \
   rm /tmp/ci.pub'
```

La **même pubkey** doit aussi être dans `~allfab/.ssh/authorized_keys` sur le bastion (`163.172.82.220`) pour que le ProxyJump fonctionne avec la même clé.

### 5. Stack nginx

Copier `docker-compose.yml` et `nginx.conf` dans `/opt/docker/apps-stack/garmin-img-forge/` puis :

```bash
cd /opt/docker/apps-stack/garmin-img-forge/
docker compose up -d
docker ps --filter name=garmin-ign-topo-map
# Attendu : status "Up ... (healthy)" sous 2 min
```

> **Note :** Avant le premier push CI, `www/` est vide → nginx renvoie 403. Normal.

### 6. Routage Traefik

Copier `03-garmin.yml` dans le répertoire dynamique de la stack Traefik Frontend. Traefik recharge automatiquement et route `maps.garmin.allfabox.fr` vers `http://10.0.100.30:8880`.

## Secrets Woodpecker CI

À créer dans l'interface Woodpecker (settings repo), visibles uniquement sur event `push` / branche `main` / pipeline `site.yml` :

| Secret                      | Valeur                                                                 |
| --------------------------- | ---------------------------------------------------------------------- |
| `deploy_ssh_host`           | `10.0.100.30`                                                          |
| `deploy_ssh_user`           | `deploy`                                                               |
| `deploy_ssh_key`            | Contenu de la clé privée `woodpecker-ci-agent` (ED25519 ou ECDSA)      |
| `deploy_ssh_bastion_host`   | `163.172.82.220`                                                       |
| `deploy_ssh_bastion_user`   | `allfab`                                                               |

## Validation locale (avant push CI)

Même syntaxe que le runner, à exécuter depuis ton poste :

```bash
mkdir -p /tmp/empty-test
rsync -az --delete --chmod=D755,F644 \
  -e "ssh -i ~/.ssh/woodpecker-ci-agent -o IdentitiesOnly=yes -J allfab@163.172.82.220" \
  /tmp/empty-test/ deploy@10.0.100.30:/opt/docker/apps-stack/garmin-img-forge/www/
```

`IdentitiesOnly=yes` est utile uniquement en local (agent SSH saturé de clés). Inutile côté runner.

## Vérification post-déploiement

```bash
# Status conteneur
docker ps --filter name=garmin-ign-topo-map

# Site répond
curl -I https://maps.garmin.allfabox.fr/
# Attendu : HTTP/2 200, server: nginx

# Proxy anti-adblock Umami (/js/)
curl -I https://maps.garmin.allfabox.fr/js/script.js
# Attendu : HTTP/2 200
```

## Fichiers

| Fichier              | Description                                                          |
| -------------------- | -------------------------------------------------------------------- |
| `docker-compose.yml` | Service nginx `garmin-ign-topo-map`, port 8880, réseau `apps`, bind mount `/opt/docker/apps-stack/...`, healthcheck wget |
| `nginx.conf`         | Config nginx : `try_files` + proxy `/js/` → Umami (anti-adblock)     |
| `03-garmin.yml`      | Config dynamique Traefik (router `maps.garmin.allfabox.fr` + service backend `http://10.0.100.30:8880`) |

## Notes

- Aucun token API Forgejo n'est nécessaire (nginx sert des fichiers statiques).
- Le pipeline ne requiert plus de bind mount côté runner : `WOODPECKER_BACKEND_DOCKER_VOLUMES` peut être nettoyé du chemin `/opt/docker/forge-stack/sites/garmin-ign-topo-map/www` une fois la bascule validée.
- Durcissement optionnel : pinner le host key du bastion dans un secret `deploy_ssh_known_hosts` plutôt que `ssh-keyscan` à la volée ; ajouter un garde-fou `test $(ls site/_site/ | wc -l) -gt 5` avant rsync pour se prémunir d'un build vide.
