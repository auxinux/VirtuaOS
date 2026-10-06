//! `virtuaos channel` — PROD / DEV update channels for VirtuaOS and the kernel.
//!
//! Single source of truth shared with the Virtua panel: `/etc/virtuaos/channel`
//!
//! ```text
//! VIRTUAOS_CHANNEL=PROD
//! KERNEL_CHANNEL=PROD
//! ```
//!
//! * `VIRTUAOS_CHANNEL` selects where `vos os` reads releases from
//!   (`dep.auxinux.ca/VIRTUAOS/` vs `dep.auxinux.ca/VIRTUAOS/DEV/`).
//! * `KERNEL_CHANNEL` selects the APT suite of the kernel repo (`kernel` vs
//!   `kernel-dev`) and therefore the meta-package (`kernel-virtua` vs
//!   `kernel-virtua-dev`).
//!
//! Missing file / key / invalid value ⇒ PROD. Unknown keys are preserved.

use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::Command;

pub const CHANNEL_FILE: &str = "/etc/virtuaos/channel";
const KERNEL_SOURCES: &str = "/etc/apt/sources.list.d/auxinux-kernel.sources";
const KEYRING: &str = "/usr/share/keyrings/virtua-archive-keyring.asc";
const APT_URI: &str = "https://dep.auxinux.ca/VIRTUA";
const LOCK_FILE: &str = "/run/virtuaos-channel.lock";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Prod,
    Dev,
}

impl Channel {
    /// Lenient parse (case-insensitive); `None` for anything but prod/dev.
    pub fn parse(s: &str) -> Option<Channel> {
        match s.trim().to_ascii_uppercase().as_str() {
            "PROD" => Some(Channel::Prod),
            "DEV" => Some(Channel::Dev),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Prod => "PROD",
            Channel::Dev => "DEV",
        }
    }

    /// Base URL of the VirtuaOS release tree (release.md + <ver>/upgrade.instruction).
    pub fn release_base_url(self) -> &'static str {
        match self {
            Channel::Prod => "https://dep.auxinux.ca/VIRTUAOS",
            Channel::Dev => "https://dep.auxinux.ca/VIRTUAOS/DEV",
        }
    }

    /// APT suite of the kernel repository.
    pub fn kernel_suite(self) -> &'static str {
        match self {
            Channel::Prod => "kernel",
            Channel::Dev => "kernel-dev",
        }
    }

    /// Kernel meta-package of this channel.
    pub fn kernel_meta(self) -> &'static str {
        match self {
            Channel::Prod => "kernel-virtua",
            Channel::Dev => "kernel-virtua-dev",
        }
    }

    pub fn badge(self) -> String {
        match self {
            Channel::Prod => "PROD".green().to_string(),
            Channel::Dev => "DEV".yellow().to_string(),
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parsed `/etc/virtuaos/channel`, keeping every other line verbatim.
#[derive(Debug, Clone)]
pub struct Channels {
    pub virtuaos: Channel,
    pub kernel: Channel,
    other_lines: Vec<String>,
}

impl Channels {
    pub fn parse(text: &str) -> Channels {
        let mut ch = Channels {
            virtuaos: Channel::Prod,
            kernel: Channel::Prod,
            other_lines: Vec::new(),
        };
        for line in text.lines() {
            match line.split_once('=') {
                Some((k, v)) if k.trim() == "VIRTUAOS_CHANNEL" => {
                    ch.virtuaos = Channel::parse(v).unwrap_or(Channel::Prod)
                }
                Some((k, v)) if k.trim() == "KERNEL_CHANNEL" => {
                    ch.kernel = Channel::parse(v).unwrap_or(Channel::Prod)
                }
                _ => {
                    if !line.trim().is_empty() {
                        ch.other_lines.push(line.to_string())
                    }
                }
            }
        }
        ch
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "VIRTUAOS_CHANNEL={}\nKERNEL_CHANNEL={}\n",
            self.virtuaos, self.kernel
        );
        for l in &self.other_lines {
            out.push_str(l);
            out.push('\n');
        }
        out
    }
}

/// Read the channel file (PROD/PROD when absent or unreadable).
pub fn read() -> Channels {
    Channels::parse(&fs::read_to_string(CHANNEL_FILE).unwrap_or_default())
}

/// Atomic write (tmp + fsync + rename), mode 0644.
pub fn write(ch: &Channels) -> Result<()> {
    write_atomic(CHANNEL_FILE, &ch.render())
}

pub fn write_atomic(path: &str, content: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let p = Path::new(path);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    let tmp = format!("{path}.tmp.{}", std::process::id());
    {
        let mut f = fs::File::create(&tmp).with_context(|| format!("cannot write {tmp}"))?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot replace {path}"))?;
    Ok(())
}

/// Is this host VirtuaOS? Any one signal is enough; `/etc/os-release` alone is
/// not trusted (a base-files upgrade can reset it to Debian).
pub fn is_virtuaos() -> bool {
    if Path::new("/etc/virtuaos-version").exists() || Path::new("/etc/virtuaos-release").exists() {
        return true;
    }
    fs::read_to_string("/etc/os-release")
        .map(|t| {
            t.lines().any(|l| {
                l.split_once('=')
                    .map(|(k, v)| k.trim() == "ID" && v.trim().trim_matches('"') == "virtuaos")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Channel a kernel release string belongs to (`7.2.9-dev-auxinux-virtua-amd64` ⇒ DEV).
pub fn kernel_release_channel(release: &str) -> Channel {
    if release.contains("-dev-") || release.ends_with("-dev") {
        Channel::Dev
    } else {
        Channel::Prod
    }
}

/// Kernel release a meta-package points to, from its `Depends: linux-image-<rel>`.
pub fn meta_target_kernel(meta: &str) -> Option<String> {
    let out = Command::new("dpkg-query")
        .args(["-W", "-f=${Status}|${Depends}", meta])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let (status, depends) = text.split_once('|')?;
    if !status.contains("installed") || status.contains("not-installed") {
        return None;
    }
    parse_image_dep(depends)
}

fn parse_image_dep(depends: &str) -> Option<String> {
    depends
        .split(',')
        .map(|d| d.trim())
        .find_map(|d| d.strip_prefix("linux-image-"))
        .map(|d| d.split_whitespace().next().unwrap_or(d).to_string())
}

// ── CLI ──────────────────────────────────────────────────────────────────────

fn require_root() -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        bail!("run as root: sudo virtuaos channel …");
    }
    Ok(())
}

fn require_virtuaos() -> Result<()> {
    if !is_virtuaos() {
        bail!("this host is not VirtuaOS — update channels are not available");
    }
    Ok(())
}

/// `virtuaos channel` — show both channels and the running kernel.
pub fn show() -> Result<()> {
    require_virtuaos()?;
    let ch = read();
    let running = crate::system::kernel_release();
    let running_ch = kernel_release_channel(&running);
    println!("{}", "── Update channels ───────────────────────".dimmed());
    println!("  {:<18} {}", "branch VirtuaOS", ch.virtuaos.badge());
    println!("  {:<18} {}", "kernel", ch.kernel.badge());
    println!("  {:<18} {} [{}]", "kernel (running)", running, running_ch.badge());
    if running_ch != ch.kernel {
        println!(
            "\n{} running kernel ({}) does not match the selected kernel channel ({}) — reboot required",
            "⚠".yellow(),
            running_ch,
            ch.kernel
        );
    }
    Ok(())
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "y" | "yes" | "o" | "oui"
    ))
}

/// Exclusive lock so two channel changes (CLI + panel) never interleave.
#[allow(dead_code)]
struct Lock(fs::File);
fn lock() -> Result<Lock> {
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(LOCK_FILE)
        .with_context(|| format!("cannot open {LOCK_FILE}"))?;
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        bail!("another channel change is already running");
    }
    Ok(Lock(f))
}

/// `virtuaos channel os prod|dev` — only rewrites the channel file; `vos os
/// upgrade` then reads releases from the matching tree.
pub fn set_os(target: Channel, yes: bool) -> Result<()> {
    require_root()?;
    require_virtuaos()?;
    let _lock = lock()?;
    let mut ch = read();
    if ch.virtuaos == target {
        println!("{} VirtuaOS branch is already {}", "✓".green(), target.badge());
        return Ok(());
    }
    if target == Channel::Dev && !yes {
        println!(
            "{} DEV = BETA pre-releases, not recommended in production.",
            "⚠".yellow()
        );
        if !confirm("Switch the VirtuaOS branch to DEV?")? {
            println!("Aborted.");
            return Ok(());
        }
    }
    let previous = ch.virtuaos;
    ch.virtuaos = target;
    write(&ch)?;
    println!(
        "{} VirtuaOS branch: {} → {}  (releases: {}/)",
        "✓".green(),
        previous.badge(),
        target.badge(),
        target.release_base_url()
    );
    if target == Channel::Prod {
        println!(
            "{} no automatic downgrade: the next `vos os upgrade` follows PROD.",
            "ℹ".blue()
        );
    }
    Ok(())
}

fn kernel_sources(suite: &str) -> String {
    format!(
        "## AuxiNux kernel repository — managed by `virtuaos channel kernel` / Virtua.\n\
         Types: deb\n\
         URIs: {APT_URI}\n\
         Suites: {suite}\n\
         Components: main\n\
         Architectures: amd64\n\
         Signed-By: {KEYRING}\n"
    )
}

fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .status()
        .with_context(|| format!("failed to launch `{program}`"))?;
    if !status.success() {
        bail!("`{program} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

/// Installed packages whose name matches a DEV kernel image/headers.
fn installed_dev_kernel_packages() -> Vec<String> {
    let out = Command::new("dpkg-query")
        .args(["-W", "-f=${Package} ${Status}\n", "linux-image-*", "linux-headers-*"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.ends_with("install ok installed"))
        .filter_map(|l| l.split_whitespace().next())
        .filter(|p| p.contains("-dev-auxinux-virtua"))
        .map(|p| p.to_string())
        .collect()
}

/// A non-DEV AuxiNux kernel image present in /boot?
fn prod_kernel_in_boot() -> bool {
    fs::read_dir("/boot")
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .any(|n| {
                    n.starts_with("vmlinuz-")
                        && n.contains("auxinux-virtua")
                        && kernel_release_channel(n.trim_start_matches("vmlinuz-")) == Channel::Prod
                })
        })
        .unwrap_or(false)
}

/// `virtuaos channel kernel prod|dev` — switch the kernel APT suite, install the
/// channel's meta-package and (for PROD) purge the DEV kernels so GRUB boots the
/// PROD one (GRUB picks the highest version: 7.2.9-dev > 7.2.4).
pub fn set_kernel(target: Channel, yes: bool) -> Result<()> {
    require_root()?;
    require_virtuaos()?;
    let _lock = lock()?;
    let mut ch = read();
    let previous = ch.kernel;
    let dev_pkgs = installed_dev_kernel_packages();

    if previous == target && (target == Channel::Dev || dev_pkgs.is_empty()) {
        println!("{} kernel channel is already {}", "✓".green(), target.badge());
        return Ok(());
    }

    println!(
        "→ kernel channel {} → {}  (suite `{}`, meta `{}`)",
        previous.badge(),
        target.badge(),
        target.kernel_suite(),
        target.kernel_meta()
    );
    if target == Channel::Prod && !dev_pkgs.is_empty() {
        println!("  DEV kernel packages that will be purged:");
        for p in &dev_pkgs {
            println!("    • {p}");
        }
    }
    if target == Channel::Dev {
        println!("{} DEV kernels are pre-releases, not recommended in production.", "⚠".yellow());
    }
    if !yes && !confirm("Apply?")? {
        println!("Aborted.");
        return Ok(());
    }

    // Going (back) to DEV: drop any pending boot-time removal of DEV kernel
    // modules left by a previous switch to PROD, or it would delete the
    // modules of the DEV kernel we are about to install.
    if target == Channel::Dev {
        let _ = fs::remove_file("/etc/tmpfiles.d/virtuaos-kernel-cleanup.conf");
    }

    // 1. Backup, 2. rewrite sources, 3. apt-get update (rollback on failure).
    let old_sources = fs::read_to_string(KERNEL_SOURCES).ok();
    write_atomic(KERNEL_SOURCES, &kernel_sources(target.kernel_suite()))?;
    if let Err(e) = run("apt-get", &["update"]) {
        match &old_sources {
            Some(s) => write_atomic(KERNEL_SOURCES, s)?,
            None => {
                let _ = fs::remove_file(KERNEL_SOURCES);
            }
        }
        bail!("apt-get update failed — kernel sources restored: {e}");
    }

    // 4. Install the channel's meta-package.
    let dpkg_opts = [
        "-o",
        "Dpkg::Options::=--force-confdef",
        "-o",
        "Dpkg::Options::=--force-confold",
    ];
    let mut args = vec!["install", "-y"];
    args.extend(dpkg_opts);
    args.push(target.kernel_meta());
    if let Err(e) = run("apt-get", &args) {
        // Keep sources and channel file consistent: back to the previous suite.
        match &old_sources {
            Some(s) => write_atomic(KERNEL_SOURCES, s)?,
            None => {
                let _ = fs::remove_file(KERNEL_SOURCES);
            }
        }
        let _ = run("apt-get", &["update"]);
        bail!("installing {} failed — kernel sources restored: {e}", target.kernel_meta());
    }

    // 5. PROD: purge DEV kernels — only once a PROD kernel is really in /boot.
    if target == Channel::Prod {
        if !prod_kernel_in_boot() {
            bail!("no PROD AuxiNux kernel found in /boot after installing kernel-virtua — DEV kernels NOT purged");
        }
        let mut purge: Vec<String> = Vec::new();
        if crate::system::pkg_version("kernel-virtua-dev").is_some() {
            purge.push("kernel-virtua-dev".into());
        }
        purge.extend(installed_dev_kernel_packages());
        if !purge.is_empty() {
            // The RUNNING kernel may be one of them: purging deletes
            // /lib/modules/<running>, and any module not loaded yet (vhost_net,
            // tun, br_netfilter, xt_*…) would fail until reboot. Keep a copy of
            // its modules and put it back; systemd-tmpfiles removes it at boot.
            let running = crate::system::kernel_release();
            let mods = format!("/lib/modules/{running}");
            let keep = format!("/lib/modules/.{running}.virtuaos-keep");
            let running_purged = kernel_release_channel(&running) == Channel::Dev
                && Path::new(&mods).is_dir();
            if running_purged {
                let _ = fs::remove_dir_all(&keep);
                run("cp", &["-a", &mods, &keep])?;
            }
            let mut args: Vec<&str> = vec!["purge", "-y"];
            args.extend(purge.iter().map(|s| s.as_str()));
            let purged = run("apt-get", &args);
            if running_purged {
                if !Path::new(&mods).exists() {
                    let _ = fs::rename(&keep, &mods);
                } else {
                    let _ = fs::remove_dir_all(&keep);
                }
                let _ = write_atomic(
                    "/etc/tmpfiles.d/virtuaos-kernel-cleanup.conf",
                    &format!(
                        "# Modules of the purged DEV kernel {running}, kept until the next boot.\nR {mods} - - - - -\n"
                    ),
                );
            }
            purged?;
        }
    }

    // 6. Bootloader + channel file.
    if let Err(e) = run("update-grub", &[]) {
        eprintln!("  {} update-grub: {e}", "warn".yellow());
    }
    ch.kernel = target;
    write(&ch)?;

    println!(
        "\n{} kernel channel: {} → {}",
        "✓".green(),
        previous.badge(),
        target.badge()
    );
    println!("{} reboot required to run the new kernel: {}", "⚠".yellow(), "sudo reboot".bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_and_values() {
        let c = Channels::parse("");
        assert_eq!((c.virtuaos, c.kernel), (Channel::Prod, Channel::Prod));
        let c = Channels::parse("VIRTUAOS_CHANNEL=dev\nKERNEL_CHANNEL=bogus\nFOO=bar\n");
        assert_eq!((c.virtuaos, c.kernel), (Channel::Dev, Channel::Prod));
        assert_eq!(c.render(), "VIRTUAOS_CHANNEL=DEV\nKERNEL_CHANNEL=PROD\nFOO=bar\n");
    }

    #[test]
    fn kernel_channel_from_release() {
        assert_eq!(kernel_release_channel("7.2.9-dev-auxinux-virtua-amd64"), Channel::Dev);
        assert_eq!(kernel_release_channel("7.2.4-auxinux-virtua-amd64"), Channel::Prod);
    }

    #[test]
    fn image_dep() {
        let d = "linux-image-7.2.4-auxinux-virtua-amd64 (= 7.2.4-auxinux-virtua-amd64.1), linux-headers-7.2.4-auxinux-virtua-amd64 (= 7.2.4-auxinux-virtua-amd64.1)";
        assert_eq!(parse_image_dep(d).as_deref(), Some("7.2.4-auxinux-virtua-amd64"));
    }
}
