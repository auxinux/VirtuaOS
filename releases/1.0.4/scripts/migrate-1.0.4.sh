#!/usr/bin/env bash
# =============================================================================
#  VirtuaOS 1.0.4-BETA — in-place fixes for hosts installed with ≤ 1.0.3
#  Run by `vos os upgrade` (downloadedScripts). Idempotent: safe to re-run
#  (`vos os upgrade --reapply`).
#
#   1. Remove the live-only NOPASSWD sudoers left on installed systems
#   2. Restore normal permissions on files the installer wrote with umask 077
#   3. Host firewall: INPUT policy DROP (was ACCEPT = everything open)
#   4. Remove the initial admin password from the Virtua .env
#   5. Update channel file (/etc/virtuaos/channel) if missing
# =============================================================================
set -euo pipefail

log() { echo "[migrate-1.0.4] $*"; }
[[ $EUID -eq 0 ]] || { echo "run as root" >&2; exit 1; }

# ── 1. sudo ──────────────────────────────────────────────────────────────────
if [[ -f /etc/sudoers.d/90-virtuaos-virtua ]]; then
    if id virtua >/dev/null 2>&1 && ! id -nG virtua | tr ' ' '\n' | grep -qx sudo; then
        usermod -aG sudo virtua
        log "virtua added to the sudo group (sudo now asks for its password)"
    fi
    rm -f /etc/sudoers.d/90-virtuaos-virtua
    log "removed NOPASSWD sudoers for virtua"
else
    log "sudoers: already clean"
fi

# ── 2. permissions (installer umask 077 leak) ────────────────────────────────
for f in /etc/fstab /etc/apt/sources.list /etc/apt/sources.list.d/debian.sources \
         /etc/default/grub /etc/sysctl.d/10-console-messages.conf \
         /etc/NetworkManager/conf.d/10-virtuaos-manage.conf \
         /etc/virtuaos-version; do
    if [[ -f "${f}" && ! -L "${f}" ]]; then
        chmod 0644 "${f}"
    fi
done
[[ -f /etc/machine-id ]] && chmod 0444 /etc/machine-id
[[ -d /etc/NetworkManager/conf.d ]] && chmod 0755 /etc/NetworkManager/conf.d
[[ -f /boot/grub/grub.cfg ]] && chmod 0444 /boot/grub/grub.cfg
log "permissions restored (fstab, apt sources, grub, machine-id…)"

# ── 3. firewall ──────────────────────────────────────────────────────────────
mkdir -p /etc/virtuaos
if [[ ! -f /etc/virtuaos/firewall.conf ]]; then
    cp /etc/virtuaos/firewall.conf.dist /etc/virtuaos/firewall.conf
    # Existing hosts may talk to other cluster nodes on ports we do not know
    # about: trust the host's own directly connected IPv4 LAN(s) on all ports so
    # the switch to INPUT DROP cannot break the cluster. Tighten later by editing
    # TRUSTED_SOURCES, then `virtuaos-firewall apply`.
    # Private ranges only: on a host with a public /24 (OVH, Hetzner…) the
    # provider's neighbours must NOT be trusted.
    lans="$(ip -4 -o route show scope link 2>/dev/null \
            | awk '$3 !~ /^(virbr|lxcbr|docker|br-|veth|tap|vnet)/ {print $1}' \
            | grep -E '^(10\.|192\.168\.|172\.(1[6-9]|2[0-9]|3[01])\.|100\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\.)' \
            | sort -u | tr '\n' ' ' | sed 's/ *$//' || true)"
    sed -i "s|^TRUSTED_SOURCES=.*|TRUSTED_SOURCES=\"${lans}\"|" /etc/virtuaos/firewall.conf
    chmod 0644 /etc/virtuaos/firewall.conf
    log "firewall.conf created (trusted LAN: ${lans:-none})"
fi
/usr/local/sbin/virtuaos-firewall apply
systemctl enable netfilter-persistent.service >/dev/null 2>&1 || true

# ── 4. initial admin password out of .env ────────────────────────────────────
systemctl daemon-reload
systemctl enable virtuaos-seed-cleanup.service >/dev/null 2>&1 || true
VIRTUAOS_SEED_CLEANUP_TIMEOUT=60 /usr/local/sbin/virtuaos-seed-cleanup || true

# ── 5. channel file ──────────────────────────────────────────────────────────
if [[ ! -f /etc/virtuaos/channel ]]; then
    printf 'VIRTUAOS_CHANNEL=PROD\nKERNEL_CHANNEL=PROD\n' > /etc/virtuaos/channel
    chmod 0644 /etc/virtuaos/channel
    log "channel file created (PROD/PROD)"
fi

log "done"
