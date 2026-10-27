# VirtuaOS — ISO hyperviseur clé en main / Turnkey hypervisor ISO

**VirtuaOS** est un **ISO d'installation bootable amd64** basé sur **Debian 13
(Trixie)**, embarquant le **noyau hyperviseur AuxiNux `7.x-auxinux-virtua`** et le
panneau de contrôle **Virtua**, pré-construit pour une installation hors ligne.

**VirtuaOS** is a bootable **amd64** installer ISO based on **Debian 13
(Trixie)**, shipping the **AuxiNux hypervisor kernel `7.x-auxinux-virtua`** and
the AuxiNux **Virtua** control panel, pre-built for offline installation.

> 🌍 **Français** (en haut) puis **English** (en bas). / *French first, English below.*

---

# 🇫🇷 Français

## Qu'est-ce que VirtuaOS ?

VirtuaOS transforme un serveur x86_64 en **plateforme de virtualisation complète**
en quelques minutes. Il embarque :

- **Debian 13 (Trixie)** comme base système, stable et familière.
- Le **noyau hyperviseur AuxiNux** (`7.x-auxinux-virtua`) optimisé pour KVM.
- Le **panneau Virtua** pré-installé : interface web pour gérer **VM (QEMU/KVM)**,
  **conteneurs LXC** et **Docker**, le tout accessible à `http://<IP>:8441`.
- Une installation **100 % hors ligne** : tout est déjà dans l'ISO.

Après l'installation et le premier démarrage, le panneau est disponible sur :
- `http://<IP>:8441` — utilisateur `admin`, mot de passe choisi pendant l'installation

## Canaux de distribution

VirtuaOS est distribué via **deux canaux de mise à jour** :

| Canal | URL | Contenu |
|-------|-----|---------|
| **Normal (stable)** | `https://dep.auxinux.ca/VIRTUAOS/` | Versions **stables** finalisées, recommandées pour la production. |
| **DEV (préversion)** | `https://dep.auxinux.ca/VIRTUAOS/DEV/` | Préversions **BETA** en cours de développement, réservées aux utilisateurs souhaitant tester les nouveautés en avant-première. |

Le canal **Normal** est destiné à la communauté. Le canal **DEV** permet d'accéder
aux versions en cours de validation avant leur passage en stable.

Sur un hôte VirtuaOS, le canal est choisi dans `/etc/virtuaos/channel` (partagé
avec le panneau Virtua) :

```bash
vos channel                 # afficher les canaux + le noyau actif
vos channel os dev          # releases VirtuaOS depuis /VIRTUAOS/DEV/
vos channel kernel prod     # noyau : suite APT kernel (kernel-virtua) ou kernel-dev
vos os upgrade              # applique la dernière release du canal (manifeste signé)
```

## Pare-feu

Politique `INPUT DROP` ; ports ouverts : 22, 80, 443, 8441 + bridges invités.
Configuration : `/etc/virtuaos/firewall.conf` puis `sudo virtuaos-firewall apply`.


## Construire l'ISO

### Prérequis

| Méthode | Prérequis |
|---------|-----------|
| **Docker** (macOS, Linux) | Docker Desktop (avec *Use Rosetta* sur Apple Silicon) |
| **Native** (Debian/Ubuntu) | `live-build`, `xorriso`, `rsync`, `debootstrap` (installés automatiquement par le script si absents) |

### Construction

```bash
# Via Docker (macOS / tout hôte avec Docker)
./build.sh

# En natif sur Debian/Ubuntu (nécessite root)
sudo ./build.sh --native
```

**Résultat :** `VirtuaOS-<version>-amd64.iso` dans le dossier courant.

### Options avancées

| Variable | Rôle |
|----------|------|
| `VIRTUA_SOURCE_DIR` | Chemin vers le clone du contrôleur Virtua (layout par défaut : `../Virtua`) |
| `OUTPUT_DIR` | Dossier de sortie de l'ISO (par défaut : dossier courant) |
| `BUILD_CONTEXT` | Dossier de travail (doit être sur un disque, pas un tmpfs) |
| `AUXINUX_KEYRING_URL` | URL de téléchargement de la clé de signature du dépôt noyau |
| `AUXINUX_KEYRING_FILE` | Chemin local vers la clé (pendant que le dépôt public n'est pas publié) |

### Flasher sur une clé USB

```bash
sudo dd if=VirtuaOS-<version>-amd64.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

(ou utiliser Balena Etcher / Rufus sous Windows).

### Démarrage

Le menu de démarrage GRUB propose :
- **Install VirtuaOS** — installation interactive (choix du disque).
- **Install VirtuaOS (auto)** — installation automatisée (premier disque disponible).
- **Live shell** — démarrage en mode live sans installation.

## Télécharger l'ISO — lien fixe et fiche

La dernière ISO publiée est toujours au même endroit (redirection 302 vers
l'ISO réelle, qui garde son vrai nom) :

| Lien fixe | Contenu |
|-----------|---------|
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.iso` | l'ISO la plus récente |
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.iso.sha256` | son empreinte (`sha256sum -c`) |
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.json` | la fiche : version, noyau, Debian, logiciels, taille, sha256 |

```bash
# curl -O garderait le nom du lien fixe : on suit la redirection pour le vrai nom
iso=$(curl -fsSI -o /dev/null -w '%{redirect_url}' https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.iso)
curl -fLO "$iso" && curl -fLO "$iso.sha256"
sha256sum -c "$(basename "$iso").sha256"
```

La fiche est tirée de l'ISO elle-même (`releases/iso-manifest.py`), au même
format que celle d'AuxiNux OS : le site os.auxinux.ca la lit, rien n'est
recopié à la main.

## Publier une ISO

Depuis une machine qui a accès SSH à `virtua-dev` (hôte de build) **et** à
`virtua-repo` (dépôt) — le transfert passe en flux par elle :

```bash
# Vérifier seulement (rien n'est modifié)
releases/publish-iso.sh --iso /home/debian/ISO/VirtuaOS-1.0.4-BETA-amd64.iso

# Publier : copie en .part, comparaison sha256, renommage, .sha256,
# puis liens fixes (nginx) et fiche JSON
releases/publish-iso.sh --iso /home/debian/ISO/VirtuaOS-1.0.4-BETA-amd64.iso --deploy

# Idem, et supprimer du dépôt l'ISO vers laquelle pointait le lien fixe
# (seulement elle, et seulement après vérification du nouveau lien)
releases/publish-iso.sh --iso /home/debian/ISO/VirtuaOS-1.0.4-BETA-amd64.iso --remove-previous --deploy

# ISO déjà en ligne (vérifiée par sha256) : refaire seulement liens et fiche
releases/publish-iso.sh --iso <chemin sur virtua-dev> --name VirtuaOS-1.0.3-BETA-amd64.iso --relink --deploy
```

Une ISO publiée n'est **jamais écrasée** : une ISO reconstruite sous un nom
déjà pris est publiée en `VirtuaOS-<version>-2-amd64.iso`, `-3`, etc. (ou
`--name`). Les liens fixes sont écrits dans
`/etc/nginx/snippets/virtuaos-iso-amd64.conf` (validé par `nginx -t`, l'ancien
est remis en cas d'échec).

## Dépôts séparés

VirtuaOS et le contrôleur Virtua sont des **projets distincts**. Le layout local
par défaut est :

```text
workspace/
├── Virtua/
└── VirtuaOS/
```

Si le clone de Virtua est ailleurs, fournissez-le explicitement :

```bash
VIRTUA_SOURCE_DIR=/path/to/Virtua ./build.sh
```

Le noyau est maintenu indépendamment ; l'ISO consomme le paquet `kernel-virtua`
publié via le dépôt APT signé.

## Intégration du noyau personnalisé

VirtuaOS **n'utilise pas** le `linux-image-amd64` standard. Le noyau AuxiNux est
branché dans le pipeline live-build :

| Élément | Rôle |
|---------|------|
| `livecd/config.sh` → `--linux-packages "kernel" --linux-flavours "virtua"` | installe le méta-paquet **`kernel-virtua`** (`= ${LB_LINUX_PACKAGES}-${FLAVOUR}`), qui tire `linux-image-<ver>-auxinux-virtua-amd64` + headers |
| `livecd/package-lists/00-base.list.chroot` | retire `linux-image-amd64` pour n'embarquer que le noyau AuxiNux |
| `livecd/archives/auxinux-kernel.list.chroot` | source APT de build pour `https://dep.auxinux.ca/VIRTUA kernel main` (retirée de l'image finale) |
| `livecd/includes.chroot/etc/apt/sources.list.d/auxinux-kernel.sources` | même dépôt, **persiste** sur le système installé pour `apt full-upgrade` du noyau |
| `build.sh` → `provision_kernel_repo_key()` | injecte la clé de signature (`.key.chroot` au build + `.asc` persistant) |
| `livecd/hooks/normal/9100-live-kernel-symlinks.hook.binary` | crée `/live/vmlinuz` + `/live/initrd.img` génériques |

Le noyau lui-même est construit et publié séparément vers le dépôt APT signé.
Le build ISO tire `kernel-virtua` de ce dépôt au moment du build et l'intègre
dans le squashfs, de sorte que l'environnement live **et** le système installé
l'exécutent tous deux.

### Clé de signature du dépôt

Par défaut `build.sh` télécharge la clé depuis
`https://dep.auxinux.ca/VIRTUA/virtua-archive-keyring.asc`.

Pendant que le dépôt public n'est pas encore publié (ex. vous construisez encore
le noyau), pointez le build vers une clé locale :

```
AUXINUX_KEYRING_FILE=/path/to/virtua-archive-keyring.asc ./build.sh
```

Vous devez quand même rendre le dépôt joignable pour `apt` afin de récupérer
`kernel-virtua` pendant la phase chroot (un miroir local/staging fonctionne via
`AUXINUX_KEYRING_URL` + édition de `archives/auxinux-kernel.list.chroot`).

---

# 🇬🇧 English

## What is VirtuaOS?

VirtuaOS turns an x86_64 system into a **complete virtualization platform** in
minutes. It ships:

- **Debian 13 (Trixie)** as a stable, minimal base.
- The **AuxiNux hypervisor kernel** (`7.x-auxinux-virtua`) optimized for KVM.
- The **Virtua control panel** pre-installed: manage **VMs (QEMU/KVM)**,
  **LXC containers** and **Docker** from `http://<ip>:8441`.
- A **fully offline** installation: everything is already inside the ISO.

After install + first boot, the panel is available at:
- `http://<IP>:8441` — user `admin`, password chosen during installation

## Distribution channels

VirtuaOS is distributed through **two update channels**:

| Channel | URL | Content |
|---------|-----|---------|
| **Normal (stable)** | `https://dep.auxinux.ca/VIRTUAOS/` | Finalized **stable** releases, recommended for production. |
| **DEV (pre-release)** | `https://dep.auxinux.ca/VIRTUAOS/DEV/` | **BETA** pre-releases under development, for users who want to test new features early. |

The **Normal** channel targets the community. The **DEV** channel gives access
to releases being validated before they move to stable.

On a VirtuaOS host the channel lives in `/etc/virtuaos/channel` (shared with the
Virtua panel):

```bash
vos channel                 # show channels + running kernel
vos channel os dev          # VirtuaOS releases from /VIRTUAOS/DEV/
vos channel kernel prod     # kernel: APT suite kernel (kernel-virtua) or kernel-dev
vos os upgrade              # apply the channel's latest release (signed manifest)
```

## Firewall

`INPUT DROP` policy; open ports: 22, 80, 443, 8441 + guest bridges.
Configure `/etc/virtuaos/firewall.conf`, then `sudo virtuaos-firewall apply`.

## Building the ISO

### Prerequisites

| Method | Prerequisites |
|--------|---------------|
| **Docker** (macOS / any) | Docker Desktop (with *Use Rosetta* on Apple Silicon) |
| **Native** (Debian/Ubuntu) | `live-build`, `xorriso`, `rsync`, `debootstrap` (auto-installed by the script if missing) |

### Build

```bash
# Via Docker (macOS / any host with Docker)
./build.sh

# Natively on Debian/Ubuntu (requires root)
sudo ./build.sh --native
```

**Output:** `VirtuaOS-<version>-amd64.iso` in the current folder.

### Advanced options

| Variable | Role |
|----------|------|
| `VIRTUA_SOURCE_DIR` | Path to the Virtua repo clone (default layout: `../Virtua`) |
| `OUTPUT_DIR` | Output folder for the ISO (default: current folder) |
| `BUILD_CONTEXT` | Working folder (must be disk-backed, not tmpfs) |
| `AUXINUX_KEYRING_URL` | URL to download the kernel repo signing key |
| `AUXINUX_KEYRING_FILE` | Local path to the key (while the public repo is not yet published) |

### Flash to a USB stick

```bash
sudo dd if=VirtuaOS-<version>-amd64.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

(or use Balena Etcher / Rufus on Windows)

### Boot options

The GRUB boot menu offers:
- **Install VirtuaOS** — interactive install (you choose the disk).
- **Install VirtuaOS (auto)** — automated install (first available disk).
- **Live shell** — boot without installing.

## Download the ISO — fixed link and manifest

The latest published ISO is always at the same URL (302 redirect to the real
ISO, which keeps its real file name):

| Fixed link | Content |
|------------|---------|
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.iso` | latest ISO |
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.iso.sha256` | its checksum (`sha256sum -c`) |
| `https://dep.auxinux.ca/DOWNLOAD/ISO/virtuaos-amd64.json` | manifest: version, kernel, Debian, software, size, sha256 |

The manifest is extracted from the ISO itself (`releases/iso-manifest.py`), in
the same format as AuxiNux OS's; os.auxinux.ca reads it.

## Publishing an ISO

From a machine with SSH access to both `virtua-dev` (build host) and
`virtua-repo` (repo host) — the transfer is streamed through it:

```bash
releases/publish-iso.sh --iso /home/debian/ISO/VirtuaOS-1.0.4-BETA-amd64.iso            # check only
releases/publish-iso.sh --iso /home/debian/ISO/VirtuaOS-1.0.4-BETA-amd64.iso --deploy   # publish
releases/publish-iso.sh --iso <path> --name <published name> --relink --deploy           # already online
```

`--remove-previous` deletes the ISO the fixed link pointed to before (and its
`.sha256`), only after the new link has been verified.

A published ISO is **never overwritten**: a rebuilt ISO whose name is taken is
published as `VirtuaOS-<version>-2-amd64.iso`, `-3`, etc. (or `--name`).

## Separate repositories

VirtuaOS and the Virtua controller are **separate projects**. Default local layout:

```text
workspace/
├── Virtua/
└── VirtuaOS/
```

If the Virtua clone is elsewhere, provide it explicitly:

```bash
VIRTUA_SOURCE_DIR=/path/to/Virtua ./build.sh
```

The kernel source is maintained independently; the ISO consumes its published
`kernel-virtua` package through the signed APT repository.

## Custom kernel integration

VirtuaOS does **not** use the stock `linux-image-amd64`. The AuxiNux kernel is
wired into the live-build pipeline:

| Piece | Role |
|-------|------|
| `livecd/config.sh` → `--linux-packages "kernel" --linux-flavours "virtua"` | installs the meta-package **`kernel-virtua`** (`= ${LB_LINUX_PACKAGES}-${FLAVOUR}`), which pulls `linux-image-<ver>-auxinux-virtua-amd64` + headers |
| `livecd/package-lists/00-base.list.chroot` | `linux-image-amd64` removed so only the AuxiNux kernel is embedded |
| `livecd/archives/auxinux-kernel.list.chroot` | build-time APT source for `https://dep.auxinux.ca/VIRTUA kernel main` (removed from the final image) |
| `livecd/includes.chroot/etc/apt/sources.list.d/auxinux-kernel.sources` | same repo, **persists** on the installed system for future `apt full-upgrade` |
| `build.sh` → `provision_kernel_repo_key()` | injects the repo signing key (build-time `.key.chroot` + persistent `.asc`) |
| `livecd/hooks/normal/9100-live-kernel-symlinks.hook.binary` | creates generic `/live/vmlinuz` + `/live/initrd.img` |

The kernel itself is built and published separately to the signed APT repository.
The ISO build pulls `kernel-virtua` from that repo at build time and bakes it
into the squashfs, so the live environment **and** the installed system both run
it.

### Repo signing key

By default `build.sh` downloads the armored key from
`https://dep.auxinux.ca/VIRTUA/virtua-archive-keyring.asc`.

While the public repo is not yet published (e.g. you are still building the
kernel), point the build at a local key instead:

```bash
AUXINUX_KEYRING_FILE=/path/to/virtua-archive-keyring.asc ./build.sh
```

You still need the repo itself reachable for `apt` to fetch `kernel-virtua`
during the chroot stage (a local/staging mirror works via `AUXINUX_KEYRING_URL`
+ editing `archives/auxinux-kernel.list.chroot`).

---

## License / Licence

This project is released under the **MIT License**. See [LICENSE](LICENSE).
