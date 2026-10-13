use std::process::Command;

fn main() {
    // Commit da mostrare in diagnostica: in CI arriva da GITHUB_SHA, in locale da git.
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let commit = std::env::var("GITHUB_SHA")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
        })
        .map(|s| s.trim().chars().take(12).collect::<String>())
        .unwrap_or_else(|| "sconosciuto".to_string());
    println!("cargo:rustc-env=FRAMENOTE_COMMIT={commit}");

    // La libreria di cattura (screencapturekit) contiene codice Swift: senza
    // questo percorso il programma non trova il runtime Swift di sistema
    // (libswift_Concurrency.dylib) e si chiude all'avvio. Le impostazioni di
    // collegamento delle dipendenze non arrivano al binario finale.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }

    tauri_build::build()
}
