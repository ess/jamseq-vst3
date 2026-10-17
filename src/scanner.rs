use jamseq_plugin_api::JamSeqPluginType;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn scan_vst3_plugins() -> Vec<(String, String, String, JamSeqPluginType)> {
    let mut results = Vec::new();
    let mut seen_paths = HashSet::new();

    let scan_paths = get_vst3_search_paths();

    for dir in scan_paths {
        if !dir.exists() {
            continue;
        }
        scan_directory(&dir, &mut results, &mut seen_paths);
    }

    results
}

fn get_vst3_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Standard environment variable
    if let Ok(vst3_env) = std::env::var("VST3_PATH") {
        for p in std::env::split_paths(&vst3_env) {
            paths.push(p);
        }
    }

    // Standard Linux search paths
    #[cfg(target_os = "linux")]
    {
        paths.push(PathBuf::from("/usr/lib/vst3"));
        paths.push(PathBuf::from("/usr/local/lib/vst3"));
        if let Some(home) = std::env::var_os("HOME") {
            let home_path = PathBuf::from(home);
            paths.push(home_path.join(".vst3"));
            paths.push(home_path.join(".local/lib/vst3"));
        }
    }

    // Standard macOS search paths
    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
        if let Some(home) = std::env::var_os("HOME") {
            paths.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
        }
    }

    // Standard Windows search paths
    #[cfg(target_os = "windows")]
    {
        if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
            paths.push(PathBuf::from(common).join("VST3"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            paths.push(PathBuf::from(local).join("Programs/Common/VST3"));
        }
    }

    // Relative project / bundled directories
    paths.push(PathBuf::from("plugins"));
    paths.push(PathBuf::from("../plugins"));

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("plugins"));
        }
    }

    paths
}

fn scan_directory(
    dir: &Path,
    results: &mut Vec<(String, String, String, JamSeqPluginType)>,
    seen_paths: &mut HashSet<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if is_vst3_bundle_or_file(&path) {
            register_vst3(&path, results, seen_paths);
        } else if path.is_dir() {
            // Scan one sub-level of nesting (e.g. /usr/lib/vst3/Vendor/Plugin.vst3)
            if let Ok(sub_entries) = fs::read_dir(&path) {
                for sub in sub_entries.flatten() {
                    let sub_path = sub.path();
                    if is_vst3_bundle_or_file(&sub_path) {
                        register_vst3(&sub_path, results, seen_paths);
                    }
                }
            }
        }
    }
}

fn is_vst3_bundle_or_file(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()) == Some("vst3")
        || path.to_string_lossy().ends_with(".vst3")
}

fn register_vst3(
    path: &Path,
    results: &mut Vec<(String, String, String, JamSeqPluginType)>,
    seen_paths: &mut HashSet<String>,
) {
    let uri_str = path.to_string_lossy().to_string();
    if seen_paths.contains(&uri_str) {
        return;
    }

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("VST3 Plugin");

    let display_name = if file_stem.starts_with("lib") {
        &file_stem[3..]
    } else {
        file_stem
    };

    let plugin_type = deduce_vst3_type(display_name);

    seen_paths.insert(uri_str.clone());
    results.push((
        display_name.to_string(),
        uri_str,
        "vst3".to_string(),
        plugin_type,
    ));
}

fn deduce_vst3_type(display_name: &str) -> JamSeqPluginType {
    let lower = display_name.to_lowercase();
    if lower.contains("effect")
        || lower.contains(" fx")
        || lower.contains("reverb")
        || lower.contains("delay")
        || lower.contains("compressor")
        || lower.contains("filter")
        || lower.contains("eq")
        || lower.contains("distortion")
    {
        JamSeqPluginType::AudioEffect
    } else if lower.contains("analyzer")
        || lower.contains("scope")
        || lower.contains("meter")
        || lower.contains("bars")
    {
        JamSeqPluginType::Analyzer
    } else {
        JamSeqPluginType::Instrument
    }
}
