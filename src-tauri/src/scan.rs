use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::model::{FileRecord, InspectResult, Kind};

const IMAGEOPTIM_ENGINES: &str = "/Applications/ImageOptim.app/Contents/Frameworks/ImageOptimGPL.framework/Versions/A/Resources";

pub fn default_output_dir() -> PathBuf {
    let base = dirs_document().unwrap_or_else(|| PathBuf::from("."));
    base.join("Pare Output")
}

fn is_zip(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

fn zip_output(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_os_string();
    let mut name = stem;
    name.push("-pare");
    path.parent().unwrap_or(path).join(name)
}

pub fn sibling_output(dir: &Path) -> PathBuf {
    let name = dir.file_name().unwrap_or_default().to_os_string();
    let mut next = name;
    next.push("-pare");
    dir.parent().unwrap_or(dir).join(next)
}

fn dirs_document() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let docs = PathBuf::from(home).join("Documents");
        if docs.is_dir() {
            return Some(docs);
        }
        return Some(PathBuf::from(std::env::var_os("HOME")?));
    }
    None
}

pub fn inspect_paths(paths: &[String]) -> InspectResult {
    let mut files = Vec::new();
    let mut roots = Vec::new();
    let mut suggested = None;
    for raw in paths {
        let path = PathBuf::from(raw);
        if path.is_dir() {
            roots.push(path.to_string_lossy().to_string());
            if suggested.is_none() && paths.len() == 1 {
                suggested = Some(sibling_output(&path).to_string_lossy().to_string());
            }
            walk_dir(&path, &path, &mut files);
        } else if is_zip(&path) {
            match expand_zip(&path) {
                Ok(dir) => {
                    roots.push(dir.to_string_lossy().to_string());
                    if suggested.is_none() && paths.len() == 1 {
                        suggested = Some(zip_output(&path).to_string_lossy().to_string());
                    }
                    walk_dir(&dir, &dir, &mut files);
                }
                Err(message) => push_note(&path, &message, &mut files),
            }
        } else if path.is_file() {
            push_file(&path, Path::new(path.file_name().unwrap_or_default()), &mut files);
        }
    }
    InspectResult {
        files,
        roots,
        suggested_output: suggested,
    }
}

fn walk_dir(root: &Path, dir: &Path, out: &mut Vec<FileRecord>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "Thumbs.db" {
            continue;
        }
        if path.is_dir() {
            walk_dir(root, &path, out);
            continue;
        }
        if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            push_file(&path, rel, out);
        }
    }
}

fn push_file(path: &Path, relative: &Path, out: &mut Vec<FileRecord>) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name == ".DS_Store" || name == "Thumbs.db" || name.starts_with('.') {
        return;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let kind = Kind::from_extension(ext);
    let input_bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let (animated, has_alpha) = match kind {
        Kind::Gif => gif_flags(path),
        Kind::Png => (false, png_has_alpha(path)),
        Kind::Svg => (false, true),
        _ => (false, false),
    };
    out.push(FileRecord {
        input_path: path.to_string_lossy().to_string(),
        relative_path: relative.to_string_lossy().replace('\\', "/"),
        input_bytes,
        kind,
        animated,
        has_alpha,
        note: String::new(),
    });
}

fn push_note(path: &Path, note: &str, out: &mut Vec<FileRecord>) {
    let name = path.file_name().unwrap_or_default();
    out.push(FileRecord {
        input_path: path.to_string_lossy().to_string(),
        relative_path: name.to_string_lossy().to_string(),
        input_bytes: fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        kind: Kind::Other,
        animated: false,
        has_alpha: false,
        note: note.into(),
    });
}

fn expand_zip(path: &Path) -> Result<PathBuf, String> {
    const MAX_FILES: usize = 500;
    const MAX_BYTES: u64 = 200 * 1024 * 1024;
    let file = File::open(path).map_err(|_| "压缩包无法打开".to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "压缩包无法打开".to_string())?;
    if archive.len() > MAX_FILES {
        return Err("压缩包太大".into());
    }
    let dest = std::env::temp_dir().join(format!(
        "pare-zip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dest).map_err(|_| "无法创建临时文件夹".to_string())?;
    let mut written = 0u64;
    let mut images = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| "压缩包无法打开".to_string())?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name().map(|item| item.to_path_buf()) else {
            let _ = fs::remove_dir_all(&dest);
            return Err("压缩包路径不安全".into());
        };
        if rel.components().any(|part| part.as_os_str() == "__MACOSX") {
            continue;
        }
        let ext = rel.extension().and_then(|item| item.to_str()).unwrap_or("");
        if !matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "svg") {
            continue;
        }
        let target = match safe_join(&dest, &rel.to_string_lossy()) {
            Ok(target) => target,
            Err(message) => {
                let _ = fs::remove_dir_all(&dest);
                return Err(message);
            }
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|_| "无法创建临时文件夹".to_string())?;
        }
        let mut output = File::create(&target).map_err(|_| "压缩包无法打开".to_string())?;
        let copied = std::io::copy(&mut entry, &mut output).map_err(|_| "压缩包无法打开".to_string())?;
        written += copied;
        if written > MAX_BYTES {
            let _ = fs::remove_dir_all(&dest);
            return Err("压缩包太大".into());
        }
        images += 1;
    }
    if images == 0 {
        let _ = fs::remove_dir_all(&dest);
        return Err("压缩包里没有支持的图片".into());
    }
    Ok(dest)
}

pub fn output_nested_in_input(output: &Path, roots: &[String]) -> bool {
    let output = normalize(output);
    roots.iter().any(|root| {
        let root = normalize(Path::new(root));
        output == root || output.starts_with(&root)
    })
}

pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut out = root.to_path_buf();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err("非法路径".into()),
        }
    }
    if !out.starts_with(root) {
        return Err("非法路径".into());
    }
    Ok(out)
}

pub fn unique_destination(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for n in 2..10_000 {
        let name = if ext.is_empty() {
            format!("{stem}-{n}")
        } else {
            format!("{stem}-{n}.{ext}")
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn png_has_alpha(path: &Path) -> bool {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut header = [0u8; 26];
    if file.read_exact(&mut header).is_err() {
        return false;
    }
    if &header[0..8] != b"\x89PNG\r\n\x1a\n" {
        return false;
    }
    let color = header[25];
    if color == 4 || color == 6 {
        return true;
    }
    let mut buf = vec![0u8; 8];
    let mut seen = 26u64;
    while seen < 1_048_576 {
        if file.read_exact(&mut buf).is_err() {
            break;
        }
        let len = u32::from_be_bytes(buf[0..4].try_into().unwrap()) as u64;
        let kind = &buf[4..8];
        if kind == b"tRNS" {
            return true;
        }
        if kind == b"IDAT" || kind == b"IEND" {
            break;
        }
        if file.seek_relative((len + 4) as i64).is_err() {
            break;
        }
        seen += 12 + len;
    }
    false
}

trait SeekRelative {
    fn seek_relative(&mut self, offset: i64) -> std::io::Result<()>;
}

impl SeekRelative for File {
    fn seek_relative(&mut self, offset: i64) -> std::io::Result<()> {
        use std::io::{Seek, SeekFrom};
        self.seek(SeekFrom::Current(offset)).map(|_| ())
    }
}

fn gif_flags(path: &Path) -> (bool, bool) {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(_) => return (false, false),
    };
    match gif_details(&data) {
        Some(info) => (info.frames > 1, info.transparent),
        None => (false, false),
    }
}

struct GifInfo {
    frames: u32,
    transparent: bool,
}

fn gif_details(data: &[u8]) -> Option<GifInfo> {
    if data.len() < 13 || &data[0..3] != b"GIF" {
        return None;
    }
    let packed = data[10];
    let mut i = 13usize;
    if packed & 0x80 != 0 {
        i += 3 * (1usize << ((packed & 0x07) + 1));
    }
    let mut frames = 0u32;
    let mut transparent = false;
    while i < data.len() {
        match data[i] {
            0x3B => break,
            0x21 => {
                if i + 1 >= data.len() {
                    break;
                }
                let label = data[i + 1];
                i += 2;
                if label == 0xF9 && i < data.len() {
                    let block = data[i] as usize;
                    if block >= 1 && i + 1 < data.len() && data[i + 1] & 0x01 != 0 {
                        transparent = true;
                    }
                }
                while i < data.len() {
                    let n = data[i] as usize;
                    i += 1;
                    if n == 0 {
                        break;
                    }
                    i = i.saturating_add(n);
                }
            }
            0x2C => {
                frames += 1;
                if i + 10 > data.len() {
                    break;
                }
                let packed = data[i + 9];
                i += 10;
                if packed & 0x80 != 0 {
                    i += 3 * (1usize << ((packed & 0x07) + 1));
                }
                if i >= data.len() {
                    break;
                }
                i += 1;
                while i < data.len() {
                    let n = data[i] as usize;
                    i += 1;
                    if n == 0 {
                        break;
                    }
                    i = i.saturating_add(n);
                }
            }
            _ => break,
        }
    }
    Some(GifInfo { frames, transparent })
}

pub fn imageoptim_engine_dir() -> &'static Path {
    Path::new(IMAGEOPTIM_ENGINES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_parent_segments() {
        let err = safe_join(Path::new("/tmp/out"), "../secret.webp");
        assert!(err.is_err());
    }

    #[test]
    fn unique_name_skips_existing() {
        let dir = std::env::temp_dir().join(format!("pare-unique-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let first = dir.join("封面.webp");
        File::create(&first).unwrap().write_all(b"a").unwrap();
        let next = unique_destination(first);
        assert!(next.ends_with("封面-2.webp"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nested_output_is_rejected() {
        assert!(output_nested_in_input(
            Path::new("/tmp/in/out"),
            &["/tmp/in".into()]
        ));
        assert!(!output_nested_in_input(
            Path::new("/tmp/in-pare"),
            &["/tmp/in".into()]
        ));
    }

    #[test]
    fn zip_extracts_images_and_rejects_slip() {
        let dir = std::env::temp_dir().join(format!("pare-zip-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let zip_path = dir.join("pics.zip");
        let file = File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("a.png", options).unwrap();
        writer.write_all(b"png").unwrap();
        writer.finish().unwrap();
        let extracted = expand_zip(&zip_path).unwrap();
        assert!(extracted.join("a.png").is_file());
        let _ = fs::remove_dir_all(&extracted);

        let bad = dir.join("bad.zip");
        let file = File::create(&bad).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer.start_file("../evil.png", options).unwrap();
        writer.write_all(b"no").unwrap();
        writer.finish().unwrap();
        assert!(expand_zip(&bad).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn counts_gif_frames() {
        let mut one = gif::Encoder::new(Vec::new(), 1, 1, &[0, 0, 0, 255, 255, 255]).unwrap();
        one.write_frame(&gif::Frame::from_rgb(1, 1, &[255, 0, 0])).unwrap();
        let bytes = one.into_inner().unwrap();
        let info = gif_details(&bytes).unwrap();
        assert_eq!(info.frames, 1);
    }
}
