//! Raccolta dei dati per la schermata "Info e diagnostica".
//! Tutti i valori sono letti dal sistema: nessun dato inventato.

use std::path::Path;

use serde::Serialize;
use sysinfo::{Disks, System};

use crate::encoder::ffmpeg::{self, FfmpegReport};
use crate::library::db::DbStatus;
use crate::paths::{anonymize_path, AppPaths, PathsView};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticInfo {
    pub generated_at: String,
    pub app: AppInfo,
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub gpus: Vec<GpuInfo>,
    pub gpu_error: Option<String>,
    pub ffmpeg: FfmpegReport,
    pub paths: PathsView,
    pub database: DbStatus,
    pub disk: DiskInfo,
    pub models: ModelsInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub commit: String,
    pub identifier: String,
    pub debug_build: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub kernel: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    pub brand: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub projects_free_bytes: Option<u64>,
    pub data_free_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
    pub installed: Vec<InstalledModel>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledModel {
    pub file_name: String,
    pub size_bytes: u64,
}

pub struct AppMeta {
    pub name: String,
    pub version: String,
    pub identifier: String,
}

pub fn collect(meta: &AppMeta, paths: &AppPaths, database: DbStatus) -> DiagnosticInfo {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let (gpus, gpu_error) = match gpus() {
        Ok(g) => (g, None),
        Err(e) => (Vec::new(), Some(e)),
    };

    let ffmpeg_report = match ffmpeg::sidecar_path() {
        Ok(p) => {
            let shown = anonymize_path(&p, paths.home_dir.as_deref());
            ffmpeg::detect(&p, shown)
        }
        Err(e) => FfmpegReport {
            found: false,
            path: String::new(),
            version: Default::default(),
            encoders: Vec::new(),
            preferred: None,
            preferred_hardware: None,
            error: Some(format!("percorso dell'app non leggibile: {e}")),
        },
    };

    let disks = Disks::new_with_refreshed_list();
    let free = |p: &Path| free_space(&disks, p);

    DiagnosticInfo {
        generated_at: now_rfc3339(),
        app: AppInfo {
            name: meta.name.clone(),
            version: meta.version.clone(),
            commit: env!("FRAMENOTE_COMMIT").to_string(),
            identifier: meta.identifier.clone(),
            debug_build: cfg!(debug_assertions),
        },
        os: OsInfo {
            name: System::name().unwrap_or_else(|| std::env::consts::OS.into()),
            version: System::long_os_version()
                .or_else(System::os_version)
                .unwrap_or_default(),
            kernel: System::kernel_version().unwrap_or_default(),
            arch: std::env::consts::ARCH.into(),
        },
        cpu: CpuInfo {
            brand: sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string())
                .unwrap_or_default(),
            physical_cores: System::physical_core_count(),
            logical_cores: sys.cpus().len(),
        },
        memory: MemoryInfo {
            total_bytes: sys.total_memory(),
            available_bytes: sys.available_memory(),
        },
        gpus,
        gpu_error,
        ffmpeg: ffmpeg_report,
        paths: paths.anonymized(),
        database,
        disk: DiskInfo {
            projects_free_bytes: free(&paths.projects_dir),
            data_free_bytes: free(&paths.data_dir),
        },
        models: models(&paths.models_dir),
    }
}

/// Adattatori grafici visti da wgpu (lo stesso motore del futuro renderer).
fn gpus() -> Result<Vec<GpuInfo>, String> {
    let result = std::panic::catch_unwind(|| {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::PRIMARY));
        let mut list: Vec<GpuInfo> = adapters
            .iter()
            .map(|a| {
                let info = a.get_info();
                GpuInfo {
                    name: info.name,
                    backend: format!("{:?}", info.backend),
                    device_type: device_type_it(info.device_type).into(),
                    driver: [info.driver, info.driver_info]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" "),
                }
            })
            .collect();
        list.dedup();
        list
    });
    result.map_err(|_| "errore durante l'enumerazione delle GPU (dettagli nel log)".to_string())
}

fn device_type_it(t: wgpu::DeviceType) -> &'static str {
    match t {
        wgpu::DeviceType::IntegratedGpu => "integrata",
        wgpu::DeviceType::DiscreteGpu => "dedicata",
        wgpu::DeviceType::VirtualGpu => "virtuale",
        wgpu::DeviceType::Cpu => "software (CPU)",
        wgpu::DeviceType::Other => "altro",
    }
}

/// Spazio libero sul disco che contiene `path` (punto di montaggio più lungo).
fn free_space(disks: &Disks, path: &Path) -> Option<u64> {
    disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}

/// Modelli presenti nella cartella `models/` (in M0 nessuno: il download arriva in M4).
pub fn models(models_dir: &Path) -> ModelsInfo {
    let mut installed: Vec<InstalledModel> = std::fs::read_dir(models_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let name = e.file_name().to_string_lossy().into_owned();
            let ext = Path::new(&name)
                .extension()
                .and_then(|x| x.to_str())
                .map(str::to_ascii_lowercase);
            (meta.is_file() && matches!(ext.as_deref(), Some("gguf" | "bin" | "onnx"))).then_some(
                InstalledModel {
                    file_name: name,
                    size_bytes: meta.len(),
                },
            )
        })
        .collect();
    installed.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    ModelsInfo { installed }
}

pub fn now_rfc3339() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modelli_assenti_elenco_vuoto() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(models(tmp.path()).installed.is_empty());
        assert!(models(&tmp.path().join("non-esiste")).installed.is_empty());
    }

    #[test]
    fn riconosce_i_file_modello() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("ggml-small.bin"), [0u8; 10]).unwrap();
        std::fs::write(tmp.path().join("qwen.GGUF"), [0u8; 4]).unwrap();
        std::fs::write(tmp.path().join("note.txt"), "x").unwrap();
        std::fs::write(tmp.path().join("parziale.gguf.part"), "x").unwrap();
        let m = models(tmp.path());
        let names: Vec<_> = m.installed.iter().map(|x| x.file_name.as_str()).collect();
        assert_eq!(names, ["ggml-small.bin", "qwen.GGUF"]);
        assert_eq!(m.installed[0].size_bytes, 10);
    }

    #[test]
    fn spazio_libero_della_cartella_temporanea() {
        let disks = Disks::new_with_refreshed_list();
        let tmp = tempfile::tempdir().unwrap();
        // In container senza dischi elencati sysinfo può non trovarne: in quel
        // caso il valore è None, mai un numero inventato.
        if let Some(free) = free_space(&disks, tmp.path()) {
            assert!(free > 0);
        }
    }
}
