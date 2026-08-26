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
- `http://<IP>:8441` — identifiants par défaut `admin` / `admin123`

## Disponibilité des versions

| Canal | Version | Description |
|-------|---------|-------------|
| **Normal (stable)** | `1.0.1` | Version stable publiée. |
| **DEV (préversion)** | `1.0.3-BETA` | La version **1.0.3** est actuellement disponible dans le canal **DEV** (préversion). Elle aligne la version affichée partout, embarque le noyau 7.1.10 et ajoute un résumé matériel à `vos status`. |

> **Note :** la version **1.0.3** est disponible dans les deux canaux —
> le canal **DEV** (`dep.auxinux.ca/VIRTUAOS/DEV/`) pour les préversions, et le
> canal **Normal** (`dep.auxinux.ca/VIRTUAOS/`) pour les versions stables.

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
- `http://<IP>:8441` — default credentials `admin` / `admin123`

## Available releases

| Channel | Version | Description |
|---------|---------|-------------|
| **Normal (stable)** | `1.0.1` | The stable public release. |
| **DEV (pre-release)** | `1.0.3-BETA` | The **1.0.3** release is currently available in the **DEV** channel (pre-release). It aligns the displayed version everywhere, ships kernel 7.1.10 and adds a hardware summary to `vos status`. |

> **Note:** release **1.0.3** is currently available in both channels — the
> **DEV** channel (`dep.auxinux.ca/VIRTUAOS/DEV/`) for pre-releases and the
> **Normal** channel (`dep.auxinux.ca/VIRTUAOS/`) for stable releases.

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
