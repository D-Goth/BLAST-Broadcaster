use std::time::Duration;

use serde::{Deserialize, Serialize};

// [UPD-01]
pub const REPO: &str = "D-Goth/BLAST-Broadcaster";
// [UPD-02]
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

// [UPD-03]
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    name: Option<String>,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

// [UPD-04]
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<String>,
    pub newer: bool,
    pub name: String,
    pub url: String,
    pub notes: String,
    pub published: Option<String>,
    pub installable: bool,
}

impl UpdateInfo {
    fn none() -> Self {
        Self {
            current: CURRENT.into(),
            latest: None,
            newer: false,
            name: String::new(),
            url: releases_page(),
            notes: String::new(),
            published: None,
            installable: false,
        }
    }
}

// [UPD-05]
pub fn releases_page() -> String {
    format!("https://github.com/{REPO}/releases")
}

// [UPD-06]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    stable: bool,
}

// [UPD-07]
pub fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim().trim_start_matches(['v', 'V']);
    let text = text.split('+').next().unwrap_or_default();
    let (core, pre) = match text.split_once('-') {
        Some((c, _)) => (c, true),
        None => (text, false),
    };
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    if parts.next().is_some() {
        return None;
    }
    Some(Version {
        major,
        minor,
        patch,
        stable: !pre,
    })
}

// [UPD-08]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(c), Some(cur)) => c > cur,
        _ => false,
    }
}

// [UPD-09]
fn evaluate(release: Release) -> UpdateInfo {
    if release.draft || release.prerelease {
        return UpdateInfo::none();
    }
    let latest = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let newer = is_newer(&latest, CURRENT);
    UpdateInfo {
        current: CURRENT.into(),
        newer,
        installable: newer && installer().is_some(),
        name: release
            .name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| release.tag_name.clone()),
        latest: Some(latest),
        url: release.html_url,
        notes: release.body.unwrap_or_default(),
        published: release.published_at,
    }
}

// [UPD-11]
pub fn installer() -> Option<&'static str> {
    use tauri::utils::{config::BundleType, platform::bundle_type};
    match bundle_type()? {
        BundleType::Nsis => Some("nsis"),
        BundleType::Msi => Some("msi"),
        BundleType::AppImage => Some("appimage"),
        BundleType::Deb => Some("deb"),
        BundleType::Rpm => Some("rpm"),
        _ => None,
    }
}

// [UPD-10]
pub fn check() -> Result<UpdateInfo, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut resp = agent
        .get(&url)
        .header("User-Agent", &format!("BLAST-Broadcaster/{CURRENT}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub injoignable : {e}"))?;
    match resp.status().as_u16() {
        200 => {}
        404 => return Ok(UpdateInfo::none()),
        403 | 429 => return Err("GitHub limite les requêtes : réessaie dans une heure.".into()),
        s => return Err(format!("réponse inattendue de GitHub (HTTP {s})")),
    }
    let release: Release = resp
        .body_mut()
        .read_json()
        .map_err(|e| format!("réponse GitHub illisible : {e}"))?;
    Ok(evaluate(release))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.1.0", "0.1.0-rc.1"));
        assert!(!is_newer("0.1.0-rc.2", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
        assert!(!is_newer("1.2.3.4", "0.1.0"));
    }

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            name: None,
            html_url: "https://github.com/D-Goth/BLAST-Broadcaster/releases/tag/x".into(),
            body: Some("notes".into()),
            published_at: None,
            draft: false,
            prerelease: false,
        }
    }

    #[test]
    fn evaluates_releases() {
        let info = evaluate(release("v99.0.0"));
        assert!(info.newer);
        assert_eq!(info.latest.as_deref(), Some("99.0.0"));
        assert_eq!(info.name, "v99.0.0");
        assert!(!evaluate(release("v0.0.0")).newer);
        let mut pre = release("v99.0.0");
        pre.prerelease = true;
        assert!(!evaluate(pre).newer);
    }
}

#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn github_answers() {
        let info = super::check().unwrap();
        assert_eq!(info.current, super::CURRENT);
    }
}
