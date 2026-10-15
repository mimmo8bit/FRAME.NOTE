//! Stato della protezione dei dati per la schermata "Privacy e sicurezza"
//! (`PIANO_SICUREZZA.md`, A2): ogni voce dice se è a posto e cosa fare.

use std::path::{Component, Path};
#[cfg(target_os = "macos")]
use std::{process::Command, time::Duration};

use serde::Serialize;

#[cfg(target_os = "macos")]
use crate::encoder::ffmpeg::run_with_timeout;
use crate::paths::{anonymize_path, is_private, AppPaths};

/// App escluse dalla registrazione dello schermo (oltre a FrameNote stessa):
/// banner delle notifiche e password manager. (identificativo, nome mostrato)
pub const CAPTURE_EXCLUDED: &[(&str, &str)] = &[
    ("com.apple.notificationcenterui", "Notifiche"),
    ("com.apple.Passwords", "Password"),
    ("com.apple.keychainaccess", "Accesso Portachiavi"),
    ("com.1password.1password", "1Password"),
    ("com.agilebits.onepassword7", "1Password 7"),
    ("com.bitwarden.desktop", "Bitwarden"),
    ("com.dashlane.dashlanephonefinal", "Dashlane"),
    ("com.lastpass.LastPass", "LastPass"),
    ("org.keepassxc.keepassxc", "KeePassXC"),
    ("com.proton.pass.electron", "Proton Pass"),
    ("in.sinew.Enpass-Desktop", "Enpass"),
];

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn is_capture_excluded(bundle_id: &str) -> bool {
    CAPTURE_EXCLUDED
        .iter()
        .any(|(id, _)| id.eq_ignore_ascii_case(bundle_id))
}

/// Esito di un controllo: `ok` a posto, `warn` da sistemare, `unknown` non verificabile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Level {
    Ok,
    Warn,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub level: Level,
    pub detail: String,
}

impl Check {
    fn new(level: Level, detail: impl Into<String>) -> Self {
        Self {
            level,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyReport {
    pub network: Check,
    pub disk_encryption: Check,
    pub projects_dir: String,
    pub cloud_sync: Check,
    pub other_users: Check,
    pub signature: Check,
    pub capture_exclusions: Vec<String>,
}

pub fn report(paths: &AppPaths) -> PrivacyReport {
    PrivacyReport {
        network: Check::new(
            Level::Ok,
            "Nessuna connessione: FrameNote non contiene componenti di rete (controllato a ogni build) e FFmpeg può usare solo file sul Mac.",
        ),
        disk_encryption: disk_encryption(),
        projects_dir: anonymize_path(&paths.projects_dir, paths.home_dir.as_deref()),
        cloud_sync: match sync_service(&paths.projects_dir.canonicalize().unwrap_or_else(|_| paths.projects_dir.clone())) {
            None => Check::new(Level::Ok, "Non sincronizzata con servizi cloud."),
            Some(service) => Check::new(
                Level::Warn,
                format!("La cartella dei progetti è sincronizzata con {service}: le registrazioni finiscono nel cloud."),
            ),
        },
        other_users: match (is_private(&paths.projects_root), is_private(&paths.data_dir)) {
            (Some(true), Some(true)) => Check::new(
                Level::Ok,
                "Progetti e dati dell'app sono leggibili solo dal tuo utente.",
            ),
            (Some(_), _) | (_, Some(_)) => Check::new(
                Level::Warn,
                "Alcune cartelle di FrameNote sono leggibili da altri utenti del computer: riapri FrameNote per sistemarle.",
            ),
            _ => Check::new(
                Level::Ok,
                "Le cartelle del profilo sono riservate al tuo utente.",
            ),
        },
        signature: signature(),
        capture_exclusions: std::iter::once("Finestre di FrameNote")
            .chain(CAPTURE_EXCLUDED.iter().map(|(_, name)| *name))
            .filter(|n| *n != "1Password 7")
            .map(String::from)
            .collect(),
    }
}

/// Servizio di sincronizzazione che contiene il percorso, se riconoscibile.
pub fn sync_service(path: &Path) -> Option<&'static str> {
    let parts: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    for (i, part) in parts.iter().enumerate() {
        let next = parts.get(i + 1).map(String::as_str).unwrap_or("");
        match part.as_str() {
            "Library" if next == "Mobile Documents" => return Some("iCloud Drive"),
            "Library" if next == "CloudStorage" => {
                let provider = parts.get(i + 2).map(String::as_str).unwrap_or("");
                return Some(match provider {
                    p if p.starts_with("Dropbox") => "Dropbox",
                    p if p.starts_with("GoogleDrive") => "Google Drive",
                    p if p.starts_with("OneDrive") => "OneDrive",
                    p if p.starts_with("Box") => "Box",
                    _ => "un servizio cloud",
                });
            }
            "iCloudDrive" => return Some("iCloud Drive"),
            p if p == "Dropbox" || p.starts_with("Dropbox (") => return Some("Dropbox"),
            p if p.starts_with("OneDrive") => return Some("OneDrive"),
            "Google Drive" | "My Drive" | "Il mio Drive" => return Some("Google Drive"),
            _ => {}
        }
    }
    None
}

#[cfg(target_os = "macos")]
const TOOL_TIMEOUT: Duration = Duration::from_secs(5);

fn disk_encryption() -> Check {
    #[cfg(target_os = "macos")]
    {
        let out = run_with_timeout(
            {
                let mut c = Command::new("/usr/bin/fdesetup");
                c.arg("status");
                c
            },
            TOOL_TIMEOUT,
        );
        match out {
            Ok(o) => parse_filevault(&o.stdout),
            Err(e) => Check::new(
                Level::Unknown,
                format!("Stato di FileVault non leggibile ({e})."),
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Check::new(
            Level::Unknown,
            "Il controllo della cifratura del disco su questo sistema arriva in una prossima versione.",
        )
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_filevault(stdout: &str) -> Check {
    if stdout.contains("FileVault is On") {
        Check::new(Level::Ok, "FileVault è attivo: il disco è cifrato.")
    } else if stdout.contains("FileVault is Off") {
        Check::new(
            Level::Warn,
            "FileVault è spento: se il Mac viene perso o rubato, le registrazioni si possono leggere togliendo il disco.",
        )
    } else if stdout.contains("Encryption in progress") {
        Check::new(
            Level::Ok,
            "FileVault si sta attivando: la cifratura è in corso.",
        )
    } else {
        Check::new(Level::Unknown, "Stato di FileVault non riconosciuto.")
    }
}

fn signature() -> Check {
    #[cfg(target_os = "macos")]
    {
        let bundle = std::env::current_exe().ok().and_then(|exe| {
            exe.ancestors()
                .find(|p| p.extension().is_some_and(|e| e == "app"))
                .map(Path::to_path_buf)
        });
        let Some(bundle) = bundle else {
            return Check::new(
                Level::Unknown,
                "FrameNote non è avviata da un pacchetto .app.",
            );
        };
        let mut cmd = Command::new("/usr/bin/codesign");
        cmd.args(["-dv", "--verbose=2"]).arg(&bundle);
        match run_with_timeout(cmd, TOOL_TIMEOUT) {
            Ok(o) => parse_codesign(&o.stderr),
            Err(e) => Check::new(Level::Unknown, format!("Firma non leggibile ({e}).")),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Check::new(
            Level::Unknown,
            "Controllo della firma disponibile solo su Mac.",
        )
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_codesign(stderr: &str) -> Check {
    let runtime = stderr
        .lines()
        .any(|l| l.contains("flags=") && l.contains("runtime"));
    let adhoc = stderr.contains("Signature=adhoc");
    match (adhoc, runtime) {
        (false, true) => Check::new(
            Level::Ok,
            "Firma stabile con Hardened Runtime: nessun altro programma può inserirsi in FrameNote e usarne il permesso di registrare lo schermo.",
        ),
        (true, _) => Check::new(
            Level::Warn,
            "Build di prova con firma provvisoria: usa la versione scaricata dalla pagina Releases.",
        ),
        (false, false) => Check::new(Level::Warn, "Firma senza Hardened Runtime."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riconosce_le_cartelle_sincronizzate() {
        let p = |s: &str| sync_service(Path::new(s));
        assert_eq!(p("/Users/dom/FrameNote/Progetti"), None);
        assert_eq!(p("/Users/dom/Documents/FrameNote/Progetti"), None);
        assert_eq!(
            p("/Users/dom/Library/Mobile Documents/com~apple~CloudDocs/FrameNote"),
            Some("iCloud Drive")
        );
        assert_eq!(
            p("/Users/dom/Library/CloudStorage/GoogleDrive-dom@x.it/My Drive/F"),
            Some("Google Drive")
        );
        assert_eq!(
            p("/Users/dom/Library/CloudStorage/Dropbox/F"),
            Some("Dropbox")
        );
        assert_eq!(p("/Users/dom/Dropbox (Personale)/F"), Some("Dropbox"));
        assert_eq!(
            p(r"C:\Users\Dom\OneDrive - Studio\FrameNote"),
            cfg!(windows).then_some("OneDrive")
        );
        assert_eq!(p("/Users/dom/OneDrive/FrameNote"), Some("OneDrive"));
    }

    #[test]
    fn legge_filevault() {
        assert_eq!(parse_filevault("FileVault is On.\n").level, Level::Ok);
        assert_eq!(parse_filevault("FileVault is Off.\n").level, Level::Warn);
        assert_eq!(parse_filevault("").level, Level::Unknown);
    }

    #[test]
    fn legge_la_firma() {
        let stable = "Executable=/Applications/FrameNote.app/Contents/MacOS/FrameNote\n\
            CodeDirectory v=20500 size=1 flags=0x10000(runtime) hashes=1+7 location=embedded\n\
            Authority=FrameNote Self-Signed\n";
        assert_eq!(parse_codesign(stable).level, Level::Ok);
        let adhoc = "CodeDirectory v=20500 size=1 flags=0x10002(adhoc,runtime)\nSignature=adhoc\n";
        assert_eq!(parse_codesign(adhoc).level, Level::Warn);
        assert_eq!(parse_codesign("flags=0x0(none)\n").level, Level::Warn);
    }

    #[test]
    fn esclusioni_dalla_registrazione() {
        assert!(is_capture_excluded("com.apple.notificationcenterui"));
        assert!(is_capture_excluded("com.1Password.1password"));
        assert!(!is_capture_excluded("com.apple.Safari"));
        let tmp = tempfile::tempdir().unwrap();
        let p = AppPaths::new(None, tmp.path(), &tmp.path().join("d"));
        let r = report(&p);
        assert_eq!(r.network.level, Level::Ok);
        assert_eq!(r.capture_exclusions[0], "Finestre di FrameNote");
        assert!(r.capture_exclusions.iter().any(|n| n == "Notifiche"));
    }
}
