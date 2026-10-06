#!/usr/bin/env bash
# =============================================================================
#  Prepare a release folder for publication:
#    1. refresh every "sha256" in <ver>/upgrade.instruction from the local
#       copies of the files/scripts it references (path under <ver>/),
#    2. sign upgrade.instruction → upgrade.instruction.asc (detached, armored)
#       with the repository key (the one published as virtua-archive-keyring.asc).
#
#  Usage:  ./sign-release.sh 1.0.4 [GPG_KEY_ID]
#  Then publish the whole <ver>/ folder (incl. .asc) under
#    https://dep.auxinux.ca/VIRTUAOS/<ver>/       (PROD)
#    https://dep.auxinux.ca/VIRTUAOS/DEV/<ver>/   (DEV)
#  and update the channel's release.md ("latest: <version>").
# =============================================================================
set -euo pipefail
cd "$(dirname "$0")"
VER="${1:?usage: $0 <version-folder> [gpg-key-id]}"
KEY="${2:-}"
DIR="${VER}"
[[ -f "${DIR}/upgrade.instruction" ]] || { echo "no ${DIR}/upgrade.instruction" >&2; exit 1; }

python3 - "${DIR}" <<'PY'
import hashlib, json, os, sys, urllib.parse
d = sys.argv[1]
p = os.path.join(d, "upgrade.instruction")
ins = json.load(open(p, encoding="utf-8"))
def local(entry):
    if "url" in entry:
        path = urllib.parse.urlparse(entry["url"]).path
        marker = "/" + d + "/"
        rel = path.split(marker, 1)[1] if marker in path else None
    else:
        rel = (entry.get("path") or entry.get("dest") or "").lstrip("/")
    return os.path.join(d, rel) if rel else None
for key in ("files", "downloads", "downloadedScripts"):
    for e in ins.get(key) or []:
        f = local(e)
        if f and os.path.isfile(f):
            e["sha256"] = hashlib.sha256(open(f, "rb").read()).hexdigest()
            print(f"  sha256 {f}")
        else:
            print(f"  WARN no local copy for {e.get('url') or e.get('path')}", file=sys.stderr)
json.dump(ins, open(p, "w", encoding="utf-8"), ensure_ascii=False, indent=2)
open(p, "a").write("\n")
PY

gpg --batch --yes ${KEY:+--local-user "${KEY}"} --armor --detach-sign \
    --output "${DIR}/upgrade.instruction.asc" "${DIR}/upgrade.instruction"
gpg --verify "${DIR}/upgrade.instruction.asc" "${DIR}/upgrade.instruction"
echo "signed: ${DIR}/upgrade.instruction.asc"
