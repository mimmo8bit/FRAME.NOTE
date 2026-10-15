//! Regole di sicurezza dell'interfaccia: la finestra di FrameNote mostra solo
//! le pagine dell'app e non può aprire siti esterni (vedi `PIANO_SICUREZZA.md`).

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Runtime, Url};

/// `true` per le pagine dell'app: `tauri://localhost` su macOS e Linux,
/// `http(s)://tauri.localhost` su Windows, il server di sviluppo solo in debug.
pub fn is_app_url(url: &Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => match url.host_str() {
            Some("tauri.localhost") => true,
            Some("localhost") => cfg!(debug_assertions) && url.port() == Some(1420),
            _ => false,
        },
        _ => false,
    }
}

/// Blocca qualsiasi navigazione fuori dall'app (link, redirect, script).
pub fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("framenote-navigazione")
        .on_navigation(|_, url| {
            let allowed = is_app_url(url);
            if !allowed {
                tracing::warn!(scheme = url.scheme(), "navigazione esterna bloccata");
            }
            allowed
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn solo_pagine_dell_app() {
        assert!(is_app_url(&url("tauri://localhost/index.html")));
        assert!(is_app_url(&url("http://tauri.localhost/")));
        assert!(is_app_url(&url("https://tauri.localhost/assets/x.js")));
        for bad in [
            "https://example.com/",
            "http://tauri.localhost.example.com/",
            "tauri://example.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,ciao",
            "mailto:a@b.it",
        ] {
            assert!(!is_app_url(&url(bad)), "{bad}");
        }
        // Server di sviluppo: solo nelle build di debug (i test lo sono).
        assert_eq!(
            is_app_url(&url("http://localhost:1420/")),
            cfg!(debug_assertions)
        );
        assert!(!is_app_url(&url("http://localhost:8080/")));
    }

    /// La CSP della finestra: niente connessioni né contenuti esterni.
    #[test]
    fn csp_chiusa() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let csp = conf["app"]["security"]["csp"].as_str().unwrap();
        let directive = |name: &str| -> Vec<String> {
            csp.split(';')
                .map(str::trim)
                .find(|d| d.split_whitespace().next() == Some(name))
                .unwrap_or_else(|| panic!("direttiva {name} mancante"))
                .split_whitespace()
                .skip(1)
                .map(String::from)
                .collect()
        };
        assert_eq!(directive("default-src"), ["'self'"]);
        assert_eq!(directive("script-src"), ["'self'"]);
        assert_eq!(directive("connect-src"), ["ipc:", "http://ipc.localhost"]);
        for name in ["object-src", "frame-src", "base-uri", "form-action"] {
            assert_eq!(directive(name), ["'none'"], "{name}");
        }
        // Nessun host esterno in nessuna direttiva.
        assert!(!csp.contains("https:") && !csp.contains("*"), "{csp}");
        // Né il protocollo asset né gli strumenti per sviluppatori.
        assert!(!csp.contains("asset:"));
        let cargo = include_str!("../Cargo.toml");
        assert!(!cargo.contains("devtools"));
    }
}
