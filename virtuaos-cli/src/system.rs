//! System inspection: versions and service health.

use crate::channel::{self, Channel};
use crate::version::extract_version;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::process::Command;

/// Application packages tracked by the updater (served from dep.auxinux.ca/VIRTUA).
/// The kernel meta-packages (kernel-virtua / kernel-virtua-dev) are shown separately.
pub const PACKAGES: &[&str] = &["auxinux-virtua", "virtuaos-cli"];

/// Services whose health we surface in `status`.
const SERVICES: &[&str] = &[
    "auxinuxvirtual-runner",
    "auxinuxvirtual-api",
    "docker",
    "virtqemud",
];

/// Run a command and capture trimmed stdout, or None on failure/empty.
fn capture(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Human-friendly OS name from `/etc/os-release`'s `PRETTY_NAME`, falling
/// back to `"VirtuaOS"` if the file is missing or unparsable.
pub fn os_pretty_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|c| {
            c.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "VirtuaOS".into())
}

pub fn kernel_release() -> String {
    capture("uname", &["-r"]).unwrap_or_else(|| "unknown".into())
}

/// Installed VirtuaOS version, preferring the `/etc/virtuaos-version` stamp
/// file over the `/etc/os-release` fields (see `os.rs::installed_version`
/// for the same logic used by `virtuaos os`).
pub fn virtuaos_release() -> Option<String> {
    if let Some(version) = std::fs::read_to_string("/etc/virtuaos-version")
        .ok()
        .and_then(|v| extract_version(v.trim()))
    {
        return Some(version);
    }

    let os_release = std::fs::read_to_string("/etc/os-release").ok()?;
    for key in ["VERSION_ID", "VERSION", "PRETTY_NAME"] {
        if let Some(value) = os_release_field(&os_release, key) {
            if let Some(version) = extract_version(&value) {
                return Some(version);
            }
        }
    }
    None
}

fn os_release_field(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (field, value) = line.split_once('=')?;
        (field == key).then(|| value.trim_matches('"').to_string())
    })
}

/// Installed version of a dpkg package, if present (removed-but-not-purged
/// packages count as absent).
pub fn pkg_version(pkg: &str) -> Option<String> {
    let out = capture("dpkg-query", &["-W", "-f=${Status}|${Version}", pkg])?;
    let (status, version) = out.split_once('|')?;
    (status.ends_with(" installed") && !version.is_empty()).then(|| version.to_string())
}

// ── Hardware detection (best-effort, never fatal) ────────────────────────────

/// CPU model name from /proc/cpuinfo (first core), trimmed.
fn cpu_model() -> Option<String> {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|c| {
            c.lines().find_map(|l| {
                l.strip_prefix("model name")
                    .or_else(|| l.strip_prefix("Model"))
                    .map(|v| v.trim_start_matches(':').trim().to_string())
            })
        })
        .filter(|s| !s.is_empty())
}

/// Number of logical CPUs (online processors).
fn cpu_count() -> Option<usize> {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .map(|c| c.lines().filter(|l| l.starts_with("processor")).count())
        .filter(|n| *n > 0)
}

/// Total RAM in MiB from /proc/meminfo.
fn mem_total_mib() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|c| {
        c.lines().find_map(|l| {
            l.strip_prefix("MemTotal:")
                .map(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
        })
        .flatten()
        .map(|kb| kb / 1024)
    })
}

/// Root filesystem size in GiB (via `df -B1 --output=size /`).
fn root_disk_gib() -> Option<f64> {
    let out = capture("df", &["-B1", "--output=size", "/"])?;
    let bytes: u64 = out.lines().nth(1)?.trim().parse().ok()?;
    Some(bytes as f64 / 1024.0 / 1024.0 / 1024.0)
}

/// KVM availability (hardware virtualization).
fn kvm_available() -> bool {
    std::path::Path::new("/dev/kvm").exists()
}

/// GPU vendor, detected from lspci (best-effort).
fn gpu_vendor() -> Option<String> {
    let out = capture("lspci", &[])?;
    let line = out.lines().find(|l| {
        let lower = l.to_lowercase();
        lower.contains("vga compatible")
            || lower.contains("3d controller")
            || lower.contains("display controller")
    })?;
    let lower = line.to_lowercase();
    if lower.contains("nvidia") {
        Some("NVIDIA".into())
    } else if lower.contains("amd") || lower.contains("ati") {
        Some("AMD".into())
    } else if lower.contains("intel") {
        Some("Intel".into())
    } else {
        Some("unknown".into())
    }
}

/// Print a compact hardware summary (CPU, RAM, disk, GPU, KVM).
fn print_hardware() {
    println!("{}", "── Hardware ──────────────────────────────".dimmed());
    if let Some(model) = cpu_model() {
        let cores = cpu_count().map(|n| format!(" · {n} cores")).unwrap_or_default();
        println!("  {:<12} {}{}", "CPU", model, cores.dimmed());
    }
    if let Some(mib) = mem_total_mib() {
        let gib = mib as f64 / 1024.0;
        println!("  {:<12} {:.1} GiB", "Memory", gib);
    }
    if let Some(gib) = root_disk_gib() {
        println!("  {:<12} {:.1} GiB", "Disk (/)", gib);
    }
    if let Some(vendor) = gpu_vendor() {
        println!("  {:<12} {}", "GPU", vendor);
    }
    println!(
        "  {:<12} {}",
        "KVM",
        if kvm_available() {
            "available".green().to_string()
        } else {
            "unavailable".red().to_string()
        }
    );
}

pub fn service_active(unit: &str) -> bool {
    capture("systemctl", &["is-active", unit]).as_deref() == Some("active")
}

/// Print VirtuaOS/kernel/tracked-package versions plus this running binary's
/// own version (`virtuaos version`).
///
/// `kernel (running)` is `uname -r`; the `kernel-virtua*` lines are the
/// META-packages (their own version + the kernel they pull in). A warning is
/// printed when the running kernel is not the one of the selected channel.
pub fn print_versions() -> Result<()> {
    const W: usize = 25;
    println!("{}", os_pretty_name().bold());
    println!(
        "  {:<W$} {}",
        "virtuaos",
        virtuaos_release().unwrap_or_else(|| "unset".dimmed().to_string())
    );
    let is_vos = channel::is_virtuaos();
    let ch = channel::read();
    if is_vos {
        println!("  {:<W$} {}", "branch VirtuaOS", ch.virtuaos.badge());
    }

    let running = kernel_release();
    let running_ch = channel::kernel_release_channel(&running);
    println!("  {:<W$} {} [{}]", "kernel (running)", running, running_ch.badge());
    if is_vos {
        println!("  {:<W$} {}", "kernel channel", ch.kernel.badge());
    }
    for meta_ch in [Channel::Prod, Channel::Dev] {
        let meta = meta_ch.kernel_meta();
        let label = format!("{meta} (meta)");
        match pkg_version(meta) {
            Some(v) => {
                let target = channel::meta_target_kernel(meta)
                    .map(|k| format!(" → {k}"))
                    .unwrap_or_default();
                println!("  {:<W$} {}{}", label, v, target.dimmed());
            }
            None if meta_ch == Channel::Prod => {
                println!("  {:<W$} {}", label, "not installed".dimmed())
            }
            None => {}
        }
    }
    for pkg in PACKAGES {
        let v = pkg_version(pkg).unwrap_or_else(|| "not installed".dimmed().to_string());
        println!("  {:<W$} {}", pkg, v);
    }
    println!(
        "  {:<W$} {} {}",
        "virtuaos (cli)",
        env!("CARGO_PKG_VERSION"),
        "(running)".dimmed()
    );

    if is_vos {
        let expected = channel::meta_target_kernel(ch.kernel.kernel_meta());
        if running_ch != ch.kernel {
            println!(
                "\n{} running kernel is {} but the kernel channel is {} — run `vos channel kernel {}` then reboot",
                "⚠".yellow(),
                running_ch,
                ch.kernel,
                ch.kernel.as_str().to_lowercase()
            );
        } else if let Some(k) = expected.filter(|k| *k != running) {
            println!(
                "\n{} running kernel {} differs from {} → {k} (reboot to switch)",
                "ℹ".blue(),
                running,
                ch.kernel.kernel_meta()
            );
        }
    }
    Ok(())
}

/// `virtuaos status` (the default command) — versions, service health, and
/// a pending-updates hint.
pub fn print_status() -> Result<()> {
    println!("{}", "── VirtuaOS ──────────────────────────────".dimmed());
    print_versions()?;

    println!();
    print_hardware();

    println!(
        "\n{}",
        "── Services ──────────────────────────────".dimmed()
    );
    for s in SERVICES {
        if service_active(s) {
            println!("  {} {:<24} {}", "●".green(), s, "active".green());
        } else {
            println!("  {} {:<24} {}", "○".red(), s, "inactive".red());
        }
    }

    println!();
    match crate::update::upgradable_count() {
        Ok(0) => println!("{} system is up to date", "✓".green()),
        Ok(n) => println!(
            "{} {} update(s) available — run {}",
            "⬆".yellow(),
            n.to_string().bold(),
            "virtuaos update".bold()
        ),
        Err(_) => println!("{} could not check for updates", "?".yellow()),
    }
    Ok(())
}
