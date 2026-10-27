#!/usr/bin/env bash
# Publie une ISO VirtuaOS construite sur l'hote de build vers le depot public,
# avec un LIEN FIXE et une FICHE JSON (meme modele que image-builder/publish-iso.sh
# du depot AuxinuxOS).
#
# POURQUOI UN SCRIPT:
#   1. L'hote de build N'A PAS de cle SSH vers l'hote de depot. Le transfert
#      passe donc EN FLUX par la machine qui lance ce script, sans fichier
#      intermediaire.
#   2. Le fichier arrive sous un nom cache (`.nom.part`) et n'est renomme
#      qu'APRES comparaison des empreintes des deux cotes. nginx sert le
#      repertoire avec autoindex: un fichier visible est telechargeable.
#   3. Une ISO publiee n'est JAMAIS ecrasee: les caches et les liens existants
#      serviraient un contenu different sous le meme nom. Une 1.0.4-BETA
#      reconstruite est publiee sous VirtuaOS-1.0.4-BETA-2-amd64.iso, etc.
#
# LIENS FIXES: le site os.auxinux.ca pointe toujours vers virtuaos-<ARCH>.iso
# (et .iso.sha256). Ce ne sont PAS des fichiers: nginx redirige (302) vers
# l'ISO reelle, par /etc/nginx/snippets/virtuaos-iso-<ARCH>.conf, que ce
# script reecrit (le server{} de dep.auxinux.ca l'inclut deja). Une
# redirection et non un lien symbolique: le fichier telecharge garde son vrai
# nom, et une reprise de telechargement ne peut pas recoller deux ISO
# differentes. A cote, la fiche virtuaos-<ARCH>.json (version, noyau, Debian,
# logiciels, taille, sha256), tiree de l'ISO par iso-manifest.py: le site
# l'affiche, plus rien n'est recopie a la main.
#
# Usage:
#   publish-iso.sh --iso CHEMIN_DISTANT [--name NOM] [--remove-previous] [--deploy]
#   publish-iso.sh --iso CHEMIN_DISTANT --name NOM_PUBLIE --relink [--remove-previous] [--deploy]
#
#   --iso     chemin de l'ISO SUR l'hote de build (obligatoire)
#   --name    nom publie (defaut: nom de l'ISO; s'il est deja pris sur le
#             depot par une autre ISO, suffixe -2, -3... avant -amd64.iso)
#   --relink  l'ISO est DEJA publiee sous NOM_PUBLIE (verifie par sha256):
#             ne refait que le .sha256 s'il manque, la fiche et les liens fixes
#   --remove-previous
#             une fois le lien fixe verifie sur la nouvelle ISO, supprime du
#             depot l'ISO VirtuaOS vers laquelle il pointait avant (et son
#             .sha256). Seulement celle-la: les autres ISO restent.
#   --deploy  effectue reellement la publication; sans cette option, le script
#             se contente de verifier et d'afficher ce qu'il ferait
#
# Variables: VIRTUAOS_BUILD_HOST (virtua-dev), VIRTUAOS_REPO_HOST (virtua-repo),
# VIRTUAOS_REPO_DIR, VIRTUAOS_PUBLIC_URL, VIRTUAOS_NGINX_SNIPPETS.
set -euo pipefail

BUILD_HOST="${VIRTUAOS_BUILD_HOST:-virtua-dev}"
REPO_HOST="${VIRTUAOS_REPO_HOST:-virtua-repo}"
REPO_DIR="${VIRTUAOS_REPO_DIR:-/var/www/dep.auxinux.ca/DOWNLOAD/ISO}"
PUBLIC_URL="${VIRTUAOS_PUBLIC_URL:-https://dep.auxinux.ca/DOWNLOAD/ISO}"
# Chemin URL du repertoire, pour les `location` nginx.
URL_PATH="/${PUBLIC_URL#https://*/}"
NGINX_SNIPPETS="${VIRTUAOS_NGINX_SNIPPETS:-/etc/nginx/snippets}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARCH="amd64"

ISO_REMOTE=""
PUBLISH_NAME=""
DEPLOY=0
RELINK=0
REMOVE_PREVIOUS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --iso)    ISO_REMOTE="${2:?--iso demande un chemin}"; shift 2 ;;
    --name)   PUBLISH_NAME="${2:?--name demande un nom}"; shift 2 ;;
    --relink) RELINK=1; shift ;;
    --remove-previous) REMOVE_PREVIOUS=1; shift ;;
    --deploy) DEPLOY=1; shift ;;
    -h|--help) sed -n '2,46p' "$0"; exit 0 ;;
    *) echo "argument inconnu: $1" >&2; exit 64 ;;
  esac
done

[[ -n "${ISO_REMOTE}" ]] || { echo "--iso est obligatoire" >&2; exit 64; }
if [[ "${RELINK}" -eq 1 && -z "${PUBLISH_NAME}" ]]; then
  echo "--relink demande --name (le nom sous lequel l'ISO est deja publiee)" >&2
  exit 64
fi
if [[ -n "${PUBLISH_NAME}" && ! "${PUBLISH_NAME}" =~ ^VirtuaOS-[A-Za-z0-9.+-]+-${ARCH}\.iso$ ]]; then
  echo "nom publie invalide: ${PUBLISH_NAME} (attendu VirtuaOS-<version>[-N]-${ARCH}.iso)" >&2
  exit 64
fi

echo "==> Source : ${BUILD_HOST}:${ISO_REMOTE}"
read -r SIZE SHA_SRC <<<"$(ssh "${BUILD_HOST}" "stat -c %s '${ISO_REMOTE}' && sha256sum '${ISO_REMOTE}' | cut -d' ' -f1" | paste -sd' ')"
[[ -n "${SIZE}" && -n "${SHA_SRC}" ]] || { echo "ISO introuvable sur ${BUILD_HOST}" >&2; exit 66; }
echo "    taille : ${SIZE} octets"
echo "    sha256 : ${SHA_SRC}"

# sha256 d'un fichier du depot (vide s'il n'existe pas).
sha_depot() { ssh "${REPO_HOST}" "sha256sum '${REPO_DIR}/$1' 2>/dev/null | cut -d' ' -f1"; }

if [[ "${RELINK}" -eq 1 ]]; then
  if [[ "$(sha_depot "${PUBLISH_NAME}")" != "${SHA_SRC}" ]]; then
    echo "ERREUR: ${PUBLISH_NAME} absente du depot ou differente de ${ISO_REMOTE}." >&2
    exit 73
  fi
  echo "==> Deja publiee, identique: seuls le .sha256, la fiche et les liens fixes seront refaits"
elif [[ -n "${PUBLISH_NAME}" ]]; then
  if ssh "${REPO_HOST}" "test -e '${REPO_DIR}/${PUBLISH_NAME}'"; then
    echo "ERREUR: ${PUBLISH_NAME} existe deja sur le depot; une ISO publiee n'est jamais ecrasee." >&2
    echo "  Si c'est la meme ISO: --relink. Sinon: un autre --name." >&2
    exit 73
  fi
else
  # Nom de l'ISO, puis -2, -3... s'il est pris. Refus d'ecraser: la meme ISO
  # deja en ligne se relie avec --relink, elle ne se republie pas.
  base="$(basename "${ISO_REMOTE}")"
  [[ "${base}" =~ ^VirtuaOS-.+-${ARCH}\.iso$ ]] \
    || { echo "nom d'ISO inattendu: ${base}; precisez --name" >&2; exit 64; }
  racine="${base%-"${ARCH}".iso}"
  PUBLISH_NAME="${base}"
  n=1
  while ssh "${REPO_HOST}" "test -e '${REPO_DIR}/${PUBLISH_NAME}'"; do
    if [[ "$(sha_depot "${PUBLISH_NAME}")" == "${SHA_SRC}" ]]; then
      echo "ERREUR: cette ISO est deja publiee sous ${PUBLISH_NAME}." >&2
      echo "  Pour la relier au lien fixe: --name ${PUBLISH_NAME} --relink" >&2
      exit 73
    fi
    n=$((n + 1))
    PUBLISH_NAME="${racine}-${n}-${ARCH}.iso"
  done
fi
echo "==> Cible  : ${REPO_HOST}:${REPO_DIR}/${PUBLISH_NAME}"

# Fiche de l'ISO, tiree de son squashfs SUR l'hote de build (lecture a
# l'offset du squashfs dans l'ISO: rien n'est copie).
echo "==> Fiche de l'ISO"
MANIFEST="$(ssh "${BUILD_HOST}" "
  set -e
  t=\$(mktemp -d); trap 'rm -rf \"\$t\"' EXIT
  cat > \"\$t/iso-manifest.py\"
  lba=\$(xorriso -indev '${ISO_REMOTE}' -find /live/filesystem.squashfs -exec report_lba -- 2>/dev/null \\
        | awk -F, '/filesystem.squashfs/ { print \$2 + 0 }')
  [ -n \"\$lba\" ]
  off=\$((lba * 2048))
  unsquashfs -q -n -o \"\$off\" -d \"\$t/r\" '${ISO_REMOTE}' var/lib/dpkg/status etc/debian_version \\
    etc/os-release usr/lib/os-release etc/virtuaos-version >/dev/null
  noyaux=\$(unsquashfs -lc -o \"\$off\" '${ISO_REMOTE}' boot | sed -n 's|.*/boot/\\(vmlinuz-[^/]*\\)\$|\\1|p')
  python3 \"\$t/iso-manifest.py\" \"\$t/r\" '${ARCH}' \$noyaux
" < "${SCRIPT_DIR}/iso-manifest.py")"
python3 -c 'import json, sys; f = json.loads(sys.argv[1])
print("    %s %s %s, noyau %s, Debian %s, construite %s" % (f["produit"], f["version"], f["architecture"],
      f["noyau"]["complet"], f["debian"]["version"], f["construite"]))
for l in f["logiciels"]: print("    - %s %s" % (l["nom"], l["version"]))' "${MANIFEST}"

# ISO vers laquelle pointe le lien fixe AVANT cette publication, lue dans
# l'extrait nginx (ligne `# virtuaos-<ARCH>.iso -> NOM`).
SNIPPET="${NGINX_SNIPPETS}/virtuaos-iso-${ARCH}.conf"
PREVIOUS="$(ssh "${REPO_HOST}" "sudo -n cat '${SNIPPET}' 2>/dev/null" \
  | sed -n "s|^# virtuaos-${ARCH}\.iso -> \(VirtuaOS-[A-Za-z0-9.+-]*-${ARCH}\.iso\)\$|\1|p" | head -n1)"
echo "==> Lien fixe actuel : ${PREVIOUS:-(aucun)}"
if [[ "${REMOVE_PREVIOUS}" -eq 1 ]]; then
  if [[ -n "${PREVIOUS}" && "${PREVIOUS}" != "${PUBLISH_NAME}" ]]; then
    echo "    ${PREVIOUS} (+ .sha256) sera supprimee apres verification du nouveau lien"
  else
    echo "    --remove-previous: rien a supprimer"
  fi
fi

if [[ "${DEPLOY}" -ne 1 ]]; then
  echo
  echo "Verifications passees. Relancez avec --deploy pour publier."
  exit 0
fi

if [[ "${RELINK}" -eq 0 ]]; then
  echo "==> Transfert en flux (${BUILD_HOST} -> ici -> ${REPO_HOST})"
  ssh "${BUILD_HOST}" "cat '${ISO_REMOTE}'" \
    | ssh "${REPO_HOST}" "cat > '${REPO_DIR}/.${PUBLISH_NAME}.part'"

  read -r SIZE_DST SHA_DST <<<"$(ssh "${REPO_HOST}" "stat -c %s '${REPO_DIR}/.${PUBLISH_NAME}.part' && sha256sum '${REPO_DIR}/.${PUBLISH_NAME}.part' | cut -d' ' -f1" | paste -sd' ')"
  echo "    taille recue : ${SIZE_DST}"
  echo "    sha256 recu  : ${SHA_DST}"

  if [[ "${SHA_SRC}" != "${SHA_DST}" || "${SIZE}" != "${SIZE_DST}" ]]; then
    echo "ERREUR: empreinte differente apres transfert. Fichier partiel supprime." >&2
    ssh "${REPO_HOST}" "rm -f '${REPO_DIR}/.${PUBLISH_NAME}.part'"
    exit 65
  fi

  echo "==> Publication (renommage atomique, sans ecraser)"
  # mv -n + verification: si un autre envoi a pris le nom entre-temps, on
  # n'ecrase pas.
  ssh "${REPO_HOST}" "
    set -e
    mv -n '${REPO_DIR}/.${PUBLISH_NAME}.part' '${REPO_DIR}/${PUBLISH_NAME}' || true
    if [ -e '${REPO_DIR}/.${PUBLISH_NAME}.part' ]; then
      rm -f '${REPO_DIR}/.${PUBLISH_NAME}.part'
      echo 'ERREUR: ${PUBLISH_NAME} est apparue pendant le transfert; rien n a ete ecrase.' >&2
      exit 73
    fi
    chmod 0664 '${REPO_DIR}/${PUBLISH_NAME}'
  "
fi

# Fichier .sha256 (avec le vrai nom): ecrit a la publication, et a la reliaison
# s'il manque ou ne correspond pas (la 1.0.3-BETA n'en avait pas).
SHA_LINE="${SHA_SRC}  ${PUBLISH_NAME}"
if [[ "$(ssh "${REPO_HOST}" "cat '${REPO_DIR}/${PUBLISH_NAME}.sha256' 2>/dev/null")" != "${SHA_LINE}" ]]; then
  echo "==> Fichier ${PUBLISH_NAME}.sha256"
  printf '%s\n' "${SHA_LINE}" | ssh "${REPO_HOST}" "
    set -e
    cat > '${REPO_DIR}/.${PUBLISH_NAME}.sha256.part'
    chmod 0664 '${REPO_DIR}/.${PUBLISH_NAME}.sha256.part'
    mv '${REPO_DIR}/.${PUBLISH_NAME}.sha256.part' '${REPO_DIR}/${PUBLISH_NAME}.sha256'
  "
fi

echo "==> Verification HTTP de l'ISO"
code="$(curl -sS -o /dev/null -w '%{http_code}' -r 0-0 "${PUBLIC_URL}/${PUBLISH_NAME}")"
len="$(curl -sSI "${PUBLIC_URL}/${PUBLISH_NAME}" | awk 'tolower($1) == "content-length:" { print $2 }' | tr -d '\r')"
echo "    HTTP ${code}, Content-Length ${len}"
[[ "${code}" == "200" || "${code}" == "206" ]] || { echo "ERREUR: l'ISO n'est pas servie" >&2; exit 70; }
[[ "${len}" == "${SIZE}" ]] || { echo "ERREUR: taille servie differente de la source" >&2; exit 70; }
[[ "$(curl -fsS "${PUBLIC_URL}/${PUBLISH_NAME}.sha256")" == "${SHA_LINE}" ]] \
  || { echo "ERREUR: ${PUBLISH_NAME}.sha256 servi incorrect" >&2; exit 70; }

# --- Liens fixes et fiche ------------------------------------------------------
STABLE="virtuaos-${ARCH}"

echo "==> Liens fixes ${STABLE}.iso -> ${PUBLISH_NAME} (${SNIPPET})"
# Ancien extrait garde de cote (.precedent, hors du motif d'include
# virtuaos-iso-*.conf): rendu si nginx refuse le nouveau. Les liens d'abord,
# la fiche ensuite: si nginx refuse, la fiche decrit encore l'ISO vers
# laquelle pointe le lien fixe.
ssh "${REPO_HOST}" "
  set -e
  err=\$(mktemp)
  sudo -n test -f '${SNIPPET}' && sudo -n cp -p '${SNIPPET}' '${SNIPPET}.precedent' || true
  sudo -n tee '${SNIPPET}' >/dev/null <<'EOF'
# ECRIT PAR releases/publish-iso.sh (depot VirtuaOS) -- ne pas modifier a la main.
# ${STABLE}.iso -> ${PUBLISH_NAME}
location = ${URL_PATH}/${STABLE}.iso {
    add_header Cache-Control \"no-cache\" always;
    return 302 ${PUBLIC_URL}/${PUBLISH_NAME};
}
location = ${URL_PATH}/${STABLE}.iso.sha256 {
    add_header Cache-Control \"no-cache\" always;
    return 302 ${PUBLIC_URL}/${PUBLISH_NAME}.sha256;
}
location = ${URL_PATH}/${STABLE}.json {
    add_header Cache-Control \"no-cache\" always;
    add_header Access-Control-Allow-Origin \"*\" always;
    default_type application/json;
    charset utf-8;
    charset_types application/json;
    try_files \$uri =404;
}
EOF
  if ! sudo -n /usr/sbin/nginx -t 2>\"\$err\"; then
    cat \"\$err\" >&2; rm -f \"\$err\"
    if sudo -n test -f '${SNIPPET}.precedent'; then sudo -n mv '${SNIPPET}.precedent' '${SNIPPET}'; else sudo -n rm -f '${SNIPPET}'; fi
    exit 1
  fi
  rm -f \"\$err\"
  sudo -n systemctl reload nginx
" || { echo "ERREUR: configuration nginx refusee; extrait precedent remis en place" >&2; exit 70; }

echo "==> Fiche ${STABLE}.json"
PUBLIE_LE="$(ssh "${REPO_HOST}" "date -u -d @\$(stat -c %Y '${REPO_DIR}/${PUBLISH_NAME}') +%Y-%m-%dT%H:%M:%SZ")"
python3 - "${MANIFEST}" "${PUBLISH_NAME}" "${SIZE}" "${SHA_SRC}" "${PUBLIE_LE}" "${PUBLIC_URL}" "${STABLE}" <<'PY' \
  | ssh "${REPO_HOST}" "cat > '${REPO_DIR}/.${STABLE}.json.part' && chmod 0664 '${REPO_DIR}/.${STABLE}.json.part' && mv '${REPO_DIR}/.${STABLE}.json.part' '${REPO_DIR}/${STABLE}.json'"
import json, sys
fiche, nom, taille, sha, publie, url, stable = sys.argv[1:]
fiche = json.loads(fiche)
fiche.update({
    "fichier": nom,
    "taille": int(taille),
    "sha256": sha,
    "publiee": publie,
    "url": f"{url}/{nom}",
    "url_sha256": f"{url}/{nom}.sha256",
    "lien_fixe": f"{url}/{stable}.iso",
    "lien_fixe_sha256": f"{url}/{stable}.iso.sha256",
})
json.dump(fiche, sys.stdout, ensure_ascii=False, indent=2)
print()
PY

echo "==> Verification des liens fixes (comme un visiteur)"
for suffixe in "" ".sha256"; do
  cible="$(curl -sS -o /dev/null -w '%{http_code} %{redirect_url}' "${PUBLIC_URL}/${STABLE}.iso${suffixe}")"
  echo "    ${STABLE}.iso${suffixe} -> ${cible}"
  [[ "${cible}" == "302 ${PUBLIC_URL}/${PUBLISH_NAME}${suffixe}" ]] \
    || { echo "ERREUR: le lien fixe ne redirige pas vers ${PUBLISH_NAME}${suffixe}" >&2
         echo "  L'extrait est-il inclus dans le server{} de dep.auxinux.ca ?" >&2
         echo "  include ${NGINX_SNIPPETS}/virtuaos-iso-*.conf;" >&2; exit 70; }
done
final="$(curl -sSL -o /dev/null -r 0-0 -w '%{http_code} %{url_effective}' "${PUBLIC_URL}/${STABLE}.iso")"
echo "    curl -L -r 0-0 ${STABLE}.iso -> ${final}"
[[ "${final}" == "206 ${PUBLIC_URL}/${PUBLISH_NAME}" ]] || { echo "ERREUR: le lien fixe n'aboutit pas a l'ISO (206)" >&2; exit 70; }
[[ "$(curl -fsSL "${PUBLIC_URL}/${STABLE}.iso.sha256")" == "${SHA_LINE}" ]] \
  || { echo "ERREUR: ${STABLE}.iso.sha256 ne donne pas \"${SHA_LINE}\"" >&2; exit 70; }
curl -fsS "${PUBLIC_URL}/${STABLE}.json" | python3 -c '
import json, sys
f = json.load(sys.stdin)
assert f["fichier"] == sys.argv[1] and f["sha256"] == sys.argv[2], "fiche servie incorrecte"
print("    " + sys.argv[3] + ".json: " + f["produit"] + " " + f["version"] + ", " + f["fichier"])' "${PUBLISH_NAME}" "${SHA_SRC}" "${STABLE}"

# Seulement maintenant: le lien fixe et la fiche servent la nouvelle ISO.
if [[ "${REMOVE_PREVIOUS}" -eq 1 && -n "${PREVIOUS}" && "${PREVIOUS}" != "${PUBLISH_NAME}" ]]; then
  echo "==> Suppression de l'ancienne ISO ${PREVIOUS}"
  ssh "${REPO_HOST}" "rm -fv '${REPO_DIR}/${PREVIOUS}' '${REPO_DIR}/${PREVIOUS}.sha256'" | sed 's/^/    /'
  code="$(curl -sS -o /dev/null -w '%{http_code}' -r 0-0 "${PUBLIC_URL}/${PREVIOUS}")"
  echo "    ${PREVIOUS} -> HTTP ${code}"
  [[ "${code}" == "404" ]] || { echo "ERREUR: ${PREVIOUS} est encore servie" >&2; exit 70; }
fi

echo
echo "Publie: ${PUBLIC_URL}/${PUBLISH_NAME}"
echo "Lien fixe: ${PUBLIC_URL}/${STABLE}.iso  (fiche: ${PUBLIC_URL}/${STABLE}.json)"
