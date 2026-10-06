//! VirtuaOS release identity (os-release, issue, issue.net).
//!
//! Single source of truth: the `/etc/virtuaos-version` marker, written by the
//! installer and by each release's `upgrade.instruction`. `virtuaos
//! apply-branding` (run by the virtuaos-cli postinst, `vos update` and `vos os
//! upgrade`) only RENDERS the identity files from that marker — it never
//! changes the version. The constant below is used only when no marker exists.
//! (Before 1.0.4 the cli stamped its own constant, so publishing a cli could
//! move every host forward — or back — regardless of the OS release.)
//!
//! KEEP IN SYNC (bump together every release) with:
//!   • VirtuaOS/VERSION
//!   • VirtuaOS/build.sh                          (VIRTUAOS_VERSION)
//!   • VirtuaOS/livecd/config.sh                  (--iso-volume / --image-name)
//!   • VirtuaOS/livecd/hooks/normal/0100-branding.hook.chroot (VIRTUAOS_VERSION)
//!   • VirtuaOS/livecd/bootloaders/grub-{pc,efi}/grub.cfg (menu labels + volume label)
//!   • VirtuaOS/livecd/includes.chroot/usr/local/sbin/virtuaos-install (VERSION)
//!
//! The login banner (/etc/profile.d/virtuaos.sh) and `virtuaos-status` are
//! DYNAMIC: they read /etc/virtuaos-version at runtime.

use crate::version;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Fallback release string when the host has no version marker.
pub const VIRTUAOS_RELEASE: &str = "1.0.4-BETA";

const ISSUE_BANNER: &str = r#"
  ██╗   ██╗██╗██████╗ ████████╗██╗   ██╗ █████╗  ██████╗ ███████╗
  ██║   ██║██║██╔══██╗╚══██╔══╝██║   ██║██╔══██╗██╔═══██╗██╔════╝
  ██║   ██║██║██████╔╝   ██║   ██║   ██║███████║██║   ██║███████╗
  ╚██╗ ██╔╝██║██╔══██╗   ██║   ██║   ██║██╔══██║██║   ██║╚════██║
   ╚████╔╝ ██║██║  ██║   ██║   ╚██████╔╝██║  ██║╚██████╔╝███████║
    ╚═══╝  ╚═╝╚═╝  ╚═╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚══════╝
"#;

/// Version from the marker, or the fallback constant.
fn current_release() -> String {
    fs::read_to_string("/etc/virtuaos-version")
        .ok()
        .and_then(|v| version::extract_version(v.trim()))
        .unwrap_or_else(|| VIRTUAOS_RELEASE.to_string())
}

/// Divert base-files' /usr/lib/os-release (target of the /etc/os-release
/// symlink) so Debian point-release upgrades stop restoring the Debian identity.
fn ensure_os_release_diverted() {
    let listed = Command::new("dpkg-divert")
        .args(["--list", "/usr/lib/os-release"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("os-release"))
        .unwrap_or(true);
    if !listed {
        let _ = Command::new("dpkg-divert")
            .args([
                "--local",
                "--rename",
                "--divert",
                "/usr/lib/os-release.debian",
                "--add",
                "/usr/lib/os-release",
            ])
            .output();
    }
}

/// (Re)write the release-identity files from the version marker. Idempotent; root only.
pub fn apply() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        bail!("this must run as root (apply-branding)");
    }
    let v = current_release();

    ensure_os_release_diverted();
    let os_release = format!(
        "PRETTY_NAME=\"VirtuaOS {v} (Debian 13 Trixie)\"\n\
         NAME=\"VirtuaOS\"\n\
         VERSION=\"{v}\"\n\
         VERSION_ID=\"{v}\"\n\
         VERSION_CODENAME=\"trixie\"\n\
         ID=\"virtuaos\"\n\
         ID_LIKE=\"debian\"\n\
         HOME_URL=\"https://auxinux.io\"\n\
         SUPPORT_URL=\"https://auxinux.io/support\"\n\
         BUG_REPORT_URL=\"https://github.com/auxinux/virtuaos/issues\"\n\
         DEBIAN_CODENAME=\"trixie\"\n"
    );
    // Write the symlink target directly when /etc/os-release is the standard
    // Debian symlink; otherwise write the regular file.
    let os_release_path = if Path::new("/etc/os-release").is_symlink() {
        "/usr/lib/os-release"
    } else {
        "/etc/os-release"
    };
    fs::write(os_release_path, os_release).with_context(|| format!("writing {os_release_path}"))?;

    if !Path::new("/etc/virtuaos-version").exists() {
        fs::write("/etc/virtuaos-version", format!("{v}\n"))
            .context("writing /etc/virtuaos-version")?;
    }
    fs::write("/etc/virtuaos-release", format!("VIRTUAOS_VERSION={v}\n"))
        .context("writing /etc/virtuaos-release")?;

    fs::write(
        "/etc/issue",
        format!("{ISSUE_BANNER}\n  Version {v}  •  Powered by Debian 13 Trixie  •  x86_64\n\n"),
    )
    .context("writing /etc/issue")?;
    fs::write(
        "/etc/issue.net",
        format!("VirtuaOS {v} — Powered by Debian 13 Trixie\nWeb Panel: http://<IP>:8441\n"),
    )
    .context("writing /etc/issue.net")?;
    Ok(())
}
