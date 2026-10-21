use std::path::PathBuf;
use std::sync::Arc;

use tauri::Manager;

use crate::batch::BatchControl;
use crate::engines::EngineSet;
use crate::model::{BatchRequest, InspectResult, JobRequest, Settings};
use crate::scan::{default_output_dir, inspect_paths, output_nested_in_input};

mod batch;
mod engines;
mod model;
mod pipeline;
mod raster;
mod scan;

#[tauri::command]
fn default_output() -> String {
    default_output_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn inspect(paths: Vec<String>) -> InspectResult {
    inspect_paths(&paths)
}

#[tauri::command]
fn validate_output(output: String, roots: Vec<String>) -> Result<(), String> {
    if output.trim().is_empty() {
        return Err("请选择输出文件夹".into());
    }
    if output_nested_in_input(PathBuf::from(&output).as_path(), &roots) {
        return Err("输出文件夹不能放在输入文件夹里面。".into());
    }
    Ok(())
}

#[tauri::command]
fn load_settings(app: tauri::AppHandle) -> Settings {
    let path = settings_path(&app);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default()
        .normalized()
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let path = settings_path(&app);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "没有写入权限".to_string())?;
    }
    let text = serde_json::to_string_pretty(&settings.normalized()).map_err(|err| err.to_string())?;
    std::fs::write(path, text).map_err(|_| "没有写入权限".to_string())
}

#[tauri::command]
fn engine_versions() -> Vec<EngineVersion> {
    EngineSet
        .versions()
        .into_iter()
        .map(|(name, version)| EngineVersion { name, version })
        .collect()
}

#[tauri::command]
fn start_batch(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<BatchControl>>,
    request: BatchRequest,
) -> Result<(), String> {
    let log = if request.settings.debug_log {
        Some(settings_path(&app).with_file_name("pare.log"))
    } else {
        None
    };
    batch::set_log(state.inner(), log);
    batch::start_batch(app, state.inner().clone(), request)
}

#[tauri::command]
fn enqueue_jobs(state: tauri::State<'_, Arc<BatchControl>>, jobs: Vec<JobRequest>) -> Result<(), String> {
    batch::enqueue(state.inner(), jobs)
}

#[tauri::command]
fn cancel_batch(state: tauri::State<'_, Arc<BatchControl>>) {
    batch::cancel_batch(state.inner());
}

#[tauri::command]
fn remove_waiting(state: tauri::State<'_, Arc<BatchControl>>, id: String) {
    batch::remove_job(state.inner(), &id);
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZipEntry {
    path: String,
    name: String,
}

#[derive(serde::Serialize)]
struct PreviewImage {
    mime: String,
    base64: String,
}

#[tauri::command]
fn open_output(path: String) -> Result<(), String> {
    let path = PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("请选择输出文件夹".into());
    }
    std::fs::create_dir_all(&path).map_err(|_| "无法创建输出文件夹".to_string())?;
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|_| "无法打开输出文件夹".to_string())
}

#[tauri::command]
fn save_copy(source: String, dest: String) -> Result<(), String> {
    let source = PathBuf::from(source.trim());
    let dest = PathBuf::from(dest.trim());
    if !source.is_file() {
        return Err("文件不存在".into());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "没有写入权限".to_string())?;
    }
    let tmp = dest.with_extension(format!(
        "{}.paretmp",
        dest.extension().and_then(|ext| ext.to_str()).unwrap_or("bin")
    ));
    std::fs::copy(&source, &tmp).map_err(|_| "没有写入权限".to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|_| "没有写入权限".to_string())?;
    Ok(())
}

#[tauri::command]
fn export_zip(dest: String, files: Vec<ZipEntry>) -> Result<(), String> {
    if files.is_empty() {
        return Err("没有可下载的图片".into());
    }
    let dest = PathBuf::from(dest.trim());
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|_| "没有写入权限".to_string())?;
    }
    let tmp = dest.with_extension("zip.paretmp");
    let file = std::fs::File::create(&tmp).map_err(|_| "没有写入权限".to_string())?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut used = std::collections::HashSet::new();
    for entry in files {
        let source = PathBuf::from(entry.path.trim());
        if !source.is_file() {
            let _ = std::fs::remove_file(&tmp);
            return Err("有文件已经不在输出文件夹里".into());
        }
        let name = unique_zip_name(&mut used, &entry.name);
        archive.start_file(name, options).map_err(|_| "无法创建压缩包".to_string())?;
        let mut input = std::fs::File::open(&source).map_err(|_| "有文件已经不在输出文件夹里".to_string())?;
        std::io::copy(&mut input, &mut archive).map_err(|_| "无法创建压缩包".to_string())?;
    }
    archive.finish().map_err(|_| "无法创建压缩包".to_string())?;
    std::fs::rename(&tmp, &dest).map_err(|_| "没有写入权限".to_string())?;
    Ok(())
}

fn unique_zip_name(used: &mut std::collections::HashSet<String>, name: &str) -> String {
    let name = name.trim().trim_start_matches(['/', '\\']);
    let name = if name.is_empty() { "image" } else { name };
    if used.insert(name.to_string()) {
        return name.to_string();
    }
    let path = std::path::Path::new(name);
    let stem = path.file_stem().and_then(|item| item.to_str()).unwrap_or("image");
    let ext = path.extension().and_then(|item| item.to_str()).unwrap_or("");
    for index in 2..10_000 {
        let next = if ext.is_empty() {
            format!("{stem}-{index}")
        } else {
            format!("{stem}-{index}.{ext}")
        };
        if used.insert(next.clone()) {
            return next;
        }
    }
    name.to_string()
}

#[tauri::command]
fn read_preview(path: String) -> Result<PreviewImage, String> {
    let path = PathBuf::from(path.trim());
    let mime = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return Err("无法预览".into()),
    };
    let bytes = std::fs::read(&path).map_err(|_| "无法预览".to_string())?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("图片太大，无法预览".into());
    }
    Ok(PreviewImage {
        mime: mime.into(),
        base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes),
    })
}

fn thumbnail_file(path: &std::path::Path) -> Result<PreviewImage, String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (mime, bytes) = if ext == "svg" {
        ("image/png", crate::raster::thumbnail_svg(path)?)
    } else if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif") {
        let mut reader = image::ImageReader::open(path)
            .map_err(|_| "无法预览".to_string())?
            .with_guessed_format()
            .map_err(|_| "无法预览".to_string())?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        let image = reader.decode().map_err(|_| "无法预览".to_string())?;
        let thumb = image.thumbnail(96, 96);
        let mut buf = std::io::Cursor::new(Vec::new());
        thumb
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|_| "无法预览".to_string())?;
        ("image/png", buf.into_inner())
    } else {
        return Err("无法预览".into());
    };
    Ok(PreviewImage {
        mime: mime.into(),
        base64: base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes),
    })
}

#[tauri::command]
fn read_thumbnail(path: String) -> Result<PreviewImage, String> {
    thumbnail_file(&PathBuf::from(path.trim()))
}

#[cfg(test)]
mod tests {
    use super::thumbnail_file;

    #[test]
    fn thumbnail_shrinks_wide_png() {
        let dir = std::env::temp_dir().join(format!("pare-thumb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("wide.png");
        let image = image::RgbImage::from_pixel(400, 200, image::Rgb([10, 40, 80]));
        image.save(&path).unwrap();
        let preview = thumbnail_file(&path).unwrap();
        assert_eq!(preview.mime, "image/png");
        let bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            preview.base64,
        )
        .unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), 96);
        assert_eq!(decoded.height(), 48);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineVersion {
    name: String,
    version: String,
}

#[cfg(target_os = "macos")]
fn set_dock_icon() {
    let path = std::env::temp_dir().join("pare-dock-icon.png");
    if std::fs::write(&path, include_bytes!("../icons/icon.png")).is_err() {
        return;
    }
    let Some(marker) = objc2::MainThreadMarker::new() else {
        return;
    };
    use objc2::AnyThread;
    let ns_path = objc2_foundation::NSString::from_str(&path.to_string_lossy());
    let Some(image) = objc2_app_kit::NSImage::initWithContentsOfFile(objc2_app_kit::NSImage::alloc(), &ns_path) else {
        return;
    };
    // 开发模式直接跑二进制，程序坞不会读 icns，这里补上。
    unsafe {
        objc2_app_kit::NSApplication::sharedApplication(marker).setApplicationIconImage(Some(&image));
    }
}

fn settings_path(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("settings.json")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Arc::new(BatchControl::new()))
        .setup(|_app| {
            #[cfg(target_os = "macos")]
            set_dock_icon();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            default_output,
            inspect,
            validate_output,
            load_settings,
            save_settings,
            engine_versions,
            start_batch,
            enqueue_jobs,
            cancel_batch,
            remove_waiting,
            open_output,
            save_copy,
            export_zip,
            read_preview,
            read_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("启动 Pare 失败");
}
