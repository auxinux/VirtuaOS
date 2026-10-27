#!/usr/bin/env python3
"""Fiche d'une ISO VirtuaOS, en JSON, tiree du systeme qu'elle contient.

POURQUOI: le site os.auxinux.ca recopiait a la main le nom de l'ISO et sa
version. publish-iso.sh publie cette fiche a cote de l'ISO; le site la lit,
avec le MEME code que la fiche AuxiNux OS (image-builder/scripts/iso-manifest.py
du depot AuxinuxOS): memes cles, en francais, aucune a renommer. Rien n'y est
saisi a la main: tout vient du squashfs de l'ISO elle-meme.

Usage: iso-manifest.py RACINE ARCH NOYAU...
  RACINE  dossier ou ont ete extraits du squashfs var/lib/dpkg/status,
          etc/debian_version, etc/os-release, usr/lib/os-release et
          etc/virtuaos-version
  ARCH    amd64
  NOYAU   noms des fichiers vmlinuz-* de /boot (le *-auxinux-virtua-* le plus
          recent est retenu, ordre de `sort -V`)

publish-iso.sh complete la fiche (fichier, taille, sha256, adresses, date de
publication). Les cles sont stables: le site en depend.
"""
import json
import re
import sys
from pathlib import Path

# Paquets dont la version interesse le public. Absents = non affiches
# (virtuaos-cli n'est pas dans toutes les ISO).
PAQUETS = {
    "auxinux-virtua": "Virtua",
    "kernel-virtua": "Noyau AuxiNux Virtua",
    "virtuaos-cli": "VirtuaOS CLI (vos)",
}

NOMS_DEBIAN = {"13": "trixie", "14": "forky", "15": "duke"}


def lire_status(chemin):
    """Paquets INSTALLES du fichier status de dpkg: {nom: champs}."""
    paquets = {}
    for bloc in chemin.read_text(encoding="utf-8").split("\n\n"):
        champs, cle = {}, None
        for ligne in bloc.splitlines():
            if ligne.startswith((" ", "\t")) and cle:
                champs[cle] += "\n" + ligne[1:]
            elif ":" in ligne:
                cle, _, valeur = ligne.partition(":")
                champs[cle] = valeur.strip()
        if champs.get("Status", "").endswith(" installed") and "Package" in champs:
            paquets[champs["Package"]] = champs
    return paquets


def lire_os_release(racine):
    """etc/os-release, ou usr/lib/os-release dont il est un lien depuis 1.0.4
    (le lien extrait du squashfs pointe hors de RACINE: on ne le suit pas)."""
    for nom in ("etc/os-release", "usr/lib/os-release"):
        chemin = racine / nom
        if chemin.is_file() and not chemin.is_symlink():
            champs = {}
            for ligne in chemin.read_text(encoding="utf-8").splitlines():
                cle, sep, valeur = ligne.partition("=")
                if sep:
                    champs[cle.strip()] = valeur.strip().strip('"')
            if champs.get("ID") == "virtuaos":
                return champs
    return {}


def cle_version(texte):
    # Meme ordre que `sort -V` pour des noms comme vmlinuz-7.2.4-auxinux-virtua-amd64.
    return [int(p) if p.isdigit() else p for p in re.split(r"(\d+)", texte)]


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    racine, arch, noyaux = Path(sys.argv[1]), sys.argv[2], sys.argv[3:]
    status = lire_status(racine / "var/lib/dpkg/status")
    os_release = lire_os_release(racine)

    # 1.0.3-BETA. etc/virtuaos-version n'est qu'un repli: en 1.0.3 il disait
    # "1.0.3", sans le -BETA du nom de l'ISO.
    version = os_release.get("VERSION_ID") or os_release.get("VERSION")
    if not version:
        fichier = racine / "etc/virtuaos-version"
        version = fichier.read_text().strip() if fichier.is_file() else None
    if not version:
        sys.exit("version de VirtuaOS introuvable dans l'ISO: fiche impossible")

    # BUILD_ID=20261006, ecrit par build.sh.
    m = re.fullmatch(r"(\d{4})(\d{2})(\d{2})", os_release.get("BUILD_ID", ""))
    construite = f"{m[1]}-{m[2]}-{m[3]}" if m else None

    noyau = sorted((n for n in noyaux if "-auxinux-virtua" in n), key=cle_version)
    noyau = noyau[-1].removeprefix("vmlinuz-") if noyau else None
    # 7.2.4-auxinux-virtua-amd64 -> 7.2.4
    noyau_court = noyau.split("-", 1)[0] if noyau else None

    debian = (racine / "etc/debian_version").read_text().strip()
    majeure = debian.split(".")[0]

    fiche = {
        "schema": 1,
        "produit": "VirtuaOS",
        "version": version,
        # Pas d'edition distincte pour VirtuaOS: meme valeur que la version.
        "edition": version,
        "architecture": arch,
        "construite": construite,
        "debian": {"version": debian, "majeure": majeure, "nom": NOMS_DEBIAN.get(majeure)},
        "noyau": {"version": noyau_court, "complet": noyau},
        "bureau": None,
        "logiciels": [
            {"paquet": p, "nom": nom, "version": status[p]["Version"]}
            for p, nom in PAQUETS.items() if p in status
        ],
        # Aucune note de version n'est embarquee dans l'ISO VirtuaOS.
        "nouveautes": [],
    }
    json.dump(fiche, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
