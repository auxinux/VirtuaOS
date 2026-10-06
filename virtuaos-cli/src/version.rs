//! VirtuaOS version strings: `1.0.4`, `1.0.4-BETA`, `1.0.4-RC2`…
//!
//! A pre-release sorts BEFORE its final release (`1.0.4-BETA` < `1.0.4`), so a
//! host on a BETA is offered the final release. Release folders on the server
//! use the numeric part only (`.../VIRTUAOS/DEV/1.0.4/upgrade.instruction`).

use std::cmp::Ordering;

/// First version-looking token in `text` (`1.2.3` with an optional
/// `-SUFFIX`), e.g. from `PRETTY_NAME="VirtuaOS 1.0.4-BETA (Debian 13)"`.
pub fn extract_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for start in 0..bytes.len() {
        if !bytes[start].is_ascii_digit() || (start > 0 && bytes[start - 1].is_ascii_alphanumeric()) {
            continue;
        }
        let mut end = start;
        let mut dots = 0;
        while end < bytes.len() && (bytes[end].is_ascii_digit() || bytes[end] == b'.') {
            if bytes[end] == b'.' {
                dots += 1;
            }
            end += 1;
        }
        let numeric = text[start..end].trim_end_matches('.');
        if dots < 1 || !numeric.contains('.') {
            continue;
        }
        let mut out = numeric.to_string();
        // Optional pre-release suffix: -[A-Za-z0-9.]+ (must start with a letter).
        let rest = &text[start + numeric.len()..];
        if let Some(sfx) = rest.strip_prefix('-') {
            let tag: String = sfx
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.')
                .collect();
            let tag = tag.trim_end_matches('.');
            if tag.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
                out.push('-');
                out.push_str(tag);
            }
        }
        return Some(out);
    }
    None
}

/// Numeric part only: `1.0.4` from `1.0.4-BETA`.
pub fn numeric(version: &str) -> &str {
    version.split('-').next().unwrap_or(version)
}

/// Pre-release tag, if any: `BETA` from `1.0.4-BETA`.
pub fn prerelease(version: &str) -> Option<&str> {
    version.split_once('-').map(|(_, p)| p).filter(|p| !p.is_empty())
}

/// Strict shape check used for user input (`--to`) before it reaches a URL/path.
pub fn is_valid(version: &str) -> bool {
    extract_version(version).as_deref() == Some(version)
}

fn parts(version: &str) -> Option<Vec<u64>> {
    let v: Option<Vec<u64>> = numeric(version)
        .split('.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u64>().ok())
        .collect();
    v.filter(|v| !v.is_empty())
}

/// Natural, case-insensitive order for pre-release tags (BETA2 < BETA10, BETA < RC).
fn cmp_tag(a: &str, b: &str) -> Ordering {
    let split = |s: &str| -> (String, u64) {
        let s = s.to_ascii_lowercase();
        let digits: String = s.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
        let num = digits.chars().rev().collect::<String>().parse().unwrap_or(0);
        (s[..s.len() - digits.len()].to_string(), num)
    };
    split(a).cmp(&split(b))
}

/// Compare two versions. `None` if either has no numeric part.
pub fn compare(a: &str, b: &str) -> Option<Ordering> {
    let a = extract_version(a).unwrap_or_else(|| a.to_string());
    let b = extract_version(b).unwrap_or_else(|| b.to_string());
    let (av, bv) = (parts(&a)?, parts(&b)?);
    for i in 0..av.len().max(bv.len()) {
        match av.get(i).unwrap_or(&0).cmp(bv.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            ord => return Some(ord),
        }
    }
    Some(match (prerelease(&a), prerelease(&b)) {
        (None, None) => Ordering::Equal,
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (Some(x), Some(y)) => cmp_tag(x, y),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract() {
        assert_eq!(extract_version("VirtuaOS 1.0.4-BETA (Debian 13 Trixie)").as_deref(), Some("1.0.4-BETA"));
        assert_eq!(extract_version("latest: 1.0.3").as_deref(), Some("1.0.3"));
        assert_eq!(extract_version("1.0.3.").as_deref(), Some("1.0.3"));
        assert_eq!(extract_version("Debian 13 Trixie"), None);
        assert_eq!(extract_version("1.0.3-").as_deref(), Some("1.0.3"));
    }

    #[test]
    fn ordering() {
        assert_eq!(compare("1.0.3-BETA", "1.0.3"), Some(Ordering::Less));
        assert_eq!(compare("1.0.3", "1.0.3-BETA"), Some(Ordering::Greater));
        assert_eq!(compare("1.0.4-BETA", "1.0.3"), Some(Ordering::Greater));
        assert_eq!(compare("1.0.4-BETA", "1.0.4-beta"), Some(Ordering::Equal));
        assert_eq!(compare("1.0.4-BETA2", "1.0.4-BETA10"), Some(Ordering::Less));
        assert_eq!(compare("1.0.4-BETA", "1.0.4-RC1"), Some(Ordering::Less));
        assert_eq!(compare("1.2", "1.2.0"), Some(Ordering::Equal));
        assert_eq!(compare("2.9", "2.10"), Some(Ordering::Less));
    }

    #[test]
    fn validity() {
        assert!(is_valid("1.0.4"));
        assert!(is_valid("1.0.4-BETA"));
        assert!(!is_valid("../1.0.4"));
        assert!(!is_valid("1.0.4/x"));
        assert_eq!(numeric("1.0.4-BETA"), "1.0.4");
    }
}
