# VirtuaOS releases (DEV / pre-release channel)

latest: 1.0.4

## 1.0.4

Final release, same content as 1.0.4-BETA (now also on the PROD channel).

## 1.0.4-BETA

Security and update-channel release.

- Removes the live-only passwordless sudo (`virtua`) left on installed hosts.
- Host firewall really enforced: INPUT policy DROP, open 22/80/443/8441 plus
  guest bridges; config in `/etc/virtuaos/firewall.conf` (`virtuaos-firewall apply`).
  Existing hosts trust their directly connected LAN(s) so cluster traffic keeps working.
- Restores normal permissions on files the installer wrote as 0600.
- Removes the initial admin password from the Virtua `.env` once the panel is initialised.
- PROD/DEV channels for VirtuaOS releases and the kernel (`vos channel`), shared
  with the Virtua panel through `/etc/virtuaos/channel`.
- `upgrade.instruction` is now signed (`upgrade.instruction.asc`) and verified.
- The version marker keeps its suffix (`1.0.4-BETA`) so the final 1.0.4 is offered.
- `/etc/os-release` is protected against base-files upgrades.

Requires virtuaos-cli ≥ 0.1.12 on the host (`apt install virtuaos-cli`), then:

    vos channel os dev
    vos os upgrade

## 1.0.3

Pre-release (BETA) aligning the displayed version everywhere (login banner, web
panel and `vos version` now read the same `/etc/virtuaos-version` marker),
shipping the AuxiNux kernel 7.1.10 and the Virtua panel 0.7.48, and adding a
hardware summary (CPU / RAM / disk / GPU / KVM) to `vos status`. The login
banner and `virtuaos-status` are now dynamic and follow upgrades automatically.
