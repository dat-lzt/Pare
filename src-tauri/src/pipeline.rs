use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::engines::{run_tool, run_tool_stdout, EngineSet, ToolRun};
use crate::model::{gifsicle_lossy_level, decide, Kind, OutputFormat, Plan, Preset, Settings};
use crate::raster::rasterize_svg;
use crate::scan::{safe_join, unique_destination};

pub struct Outcome {
    pub status: &'static str,
    pub output_bytes: Option<u64>,
    pub delta_ratio: Option<f64>,
    pub note: String,
    pub output_path: Option<String>,
}

pub fn process_file(
    engines: &EngineSet,
    input: &Path,
    relative: &str,
    kind: Kind,
    animated: bool,
    input_bytes: u64,
    settings: &Settings,
    output_dir: &Path,
    cancel: &AtomicBool,
) -> Outcome {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return cancelled();
    }
    let settings = settings.clone().normalized();
    match decide(kind, animated, &settings) {
        Plan::Unsupported => {
            let mut outcome = blank(format!(
                "不支持 .{}",
                input.extension().and_then(|e| e.to_str()).unwrap_or("")
            ));
            outcome.status = "unsupported";
            outcome
        }
        Plan::Fail(reason) => failed(reason),
        Plan::Optimize(kind) => optimize(engines, input, relative, kind, input_bytes, &settings, output_dir, cancel),
        Plan::Transcode { to, first_frame } => transcode(
            engines,
            input,
            relative,
            kind,
            input_bytes,
            &settings,
            output_dir,
            to,
            first_frame,
            cancel,
        ),
    }
}

fn optimize(
    engines: &EngineSet,
    input: &Path,
    relative: &str,
    kind: Kind,
    input_bytes: u64,
    settings: &Settings,
    output_dir: &Path,
    cancel: &AtomicBool,
) -> Outcome {
    let temp = match temp_dir() {
        Ok(path) => path,
        Err(err) => return failed(err),
    };
    let guard = TempDir(temp.clone());
    let ext = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or(kind.extension());
    let current = temp.join(format!("current.{ext}"));
    if fs::copy(input, &current).is_err() {
        return failed("这张图无法读取");
    }
    let mut note = String::new();
    let mut ran = false;
    let mut missing: Option<String> = None;
    let steps: Vec<String> = match kind {
        Kind::Jpeg => jpeg_steps(settings),
        Kind::Png => png_steps(settings),
        Kind::Gif => gif_steps(settings),
        Kind::Svg => svg_steps(settings),
        Kind::Other => Vec::new(),
    };
    if steps.is_empty() {
        drop(guard);
        return write_copy(input, relative, kind, OutputFormat::Original, input_bytes, output_dir, "未变小");
    }
    for step in steps {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return cancelled();
        }
        let tool = match engines.require(&step) {
            Ok(path) => path,
            Err(_) => {
                missing = Some(format!("缺少引擎 {step}"));
                continue;
            }
        };
        let candidate = temp.join(format!("{step}.{ext}"));
        let timeout = if step == "zopflipng" {
            Some(Duration::from_secs(60))
        } else {
            None
        };
        let result = run_step(&step, &tool, &current, &candidate, settings, timeout, cancel);
        match result {
            ToolRun::Cancelled => return cancelled(),
            ToolRun::Ok => {
                ran = true;
                if smaller(&candidate, &current) {
                    let _ = fs::copy(&candidate, &current);
                    note = step;
                }
            }
            ToolRun::TimedOut | ToolRun::Failed(_) => {}
        }
    }
    if !ran {
        return failed(missing.unwrap_or_else(|| "这张图无法读取".into()));
    }
    let current_bytes = fs::metadata(&current).map(|m| m.len()).unwrap_or(input_bytes);
    if current_bytes < input_bytes {
        match publish(&current, relative, OutputFormat::Original, kind, output_dir) {
            Ok(dest) => done(input_bytes, &dest, &note),
            Err(err) => failed(err),
        }
    } else {
        drop(guard);
        write_copy(input, relative, kind, OutputFormat::Original, input_bytes, output_dir, "未变小")
    }
}

fn jpeg_steps(settings: &Settings) -> Vec<String> {
    let mut steps = Vec::new();
    if settings.engines.jpegtran {
        steps.push("jpegtran".into());
    }
    if settings.lossy && settings.engines.jpegoptim {
        steps.push("jpegoptim".into());
    }
    steps
}

fn png_steps(settings: &Settings) -> Vec<String> {
    let mut steps = Vec::new();
    if settings.engines.oxipng {
        steps.push("oxipng".into());
    }
    if settings.lossy && settings.engines.pngquant {
        steps.push("pngquant".into());
    }
    if settings.engines.zopflipng {
        steps.push("zopflipng".into());
    }
    steps
}

fn gif_steps(settings: &Settings) -> Vec<String> {
    if settings.engines.gifsicle {
        vec!["gifsicle".into()]
    } else {
        Vec::new()
    }
}

fn svg_steps(settings: &Settings) -> Vec<String> {
    if settings.engines.svgcleaner {
        vec!["svgcleaner".into()]
    } else {
        Vec::new()
    }
}

fn run_step(
    name: &str,
    bin: &Path,
    src: &Path,
    dest: &Path,
    settings: &Settings,
    timeout: Option<Duration>,
    cancel: &AtomicBool,
) -> ToolRun {
    let src_s = src.to_string_lossy().to_string();
    let dest_s = dest.to_string_lossy().to_string();
    let args = match name {
        "jpegtran" => vec![
            "-copy".into(),
            "none".into(),
            "-optimize".into(),
            "-progressive".into(),
            "-outfile".into(),
            dest_s,
            src_s,
        ],
        "jpegoptim" => {
            if fs::copy(src, dest).is_err() {
                return ToolRun::Failed("无法写入临时文件".into());
            }
            vec![
                "--strip-all".into(),
                "--max=80".into(),
                dest.to_string_lossy().to_string(),
            ]
        }
        "oxipng" => vec![
            "-o".into(),
            "4".into(),
            "--strip".into(),
            "safe".into(),
            "--out".into(),
            dest_s,
            src_s,
        ],
        "pngquant" => vec![
            "--quality=80-100".into(),
            "--skip-if-larger".into(),
            "--force".into(),
            "--output".into(),
            dest_s,
            src_s,
        ],
        "zopflipng" => vec!["-y".into(), src_s, dest_s],
        "gifsicle" => {
            let mut args = vec!["-O3".into()];
            if settings.lossy {
                let level = gifsicle_lossy_level(80);
                if level > 0 {
                    args.push(format!("--lossy={level}"));
                }
            }
            args.push(src_s);
            args.push("-o".into());
            args.push(dest_s);
            args
        }
        "svgcleaner" => vec![src_s, dest_s],
        _ => return ToolRun::Failed(format!("未知引擎 {name}")),
    };
    let result = run_tool(bin, &args, cancel, timeout);
    if matches!(result, ToolRun::Ok) && !dest.is_file() {
        return ToolRun::Failed("没有写出结果".into());
    }
    result
}

fn transcode(
    engines: &EngineSet,
    input: &Path,
    relative: &str,
    kind: Kind,
    input_bytes: u64,
    settings: &Settings,
    output_dir: &Path,
    to: OutputFormat,
    first_frame: bool,
    cancel: &AtomicBool,
) -> Outcome {
    let temp = match temp_dir() {
        Ok(path) => path,
        Err(message) => return failed(message),
    };
    let _guard = TempDir(temp.clone());
    let prepared = if first_frame {
        match extract_first_frame(input, &temp) {
            Ok(path) => path,
            Err(message) => return failed(message),
        }
    } else {
        input.to_path_buf()
    };
    let prepared_kind = if first_frame { Kind::Gif } else { kind };
    let encoded = temp.join(match to {
        OutputFormat::Webp => "out.webp",
        OutputFormat::Jpeg => "out.jpg",
        OutputFormat::Png => "out.png",
        OutputFormat::Original => "out.bin",
    });
    let note = match to {
        OutputFormat::Webp => encode_webp(engines, &prepared, prepared_kind, settings, &encoded, cancel),
        OutputFormat::Jpeg => encode_jpeg(engines, &prepared, prepared_kind, settings, &temp, &encoded, cancel),
        OutputFormat::Png => encode_png(engines, &prepared, prepared_kind, settings, &temp, &encoded, cancel),
        OutputFormat::Original => Err(ToolRun::Failed("无法转换".into())),
    };
    let note = match note {
        Ok(note) => note,
        Err(ToolRun::Cancelled) => return cancelled(),
        Err(ToolRun::Failed(message)) => return failed(message),
        Err(ToolRun::TimedOut) => return failed("处理超时"),
        Err(ToolRun::Ok) => return failed("无法转换"),
    };
    match publish(&encoded, relative, to, kind, output_dir) {
        Ok(dest) => {
            let mut outcome = done(input_bytes, &dest, &note);
            if first_frame {
                outcome.status = "first-frame";
            }
            outcome
        }
        Err(message) => failed(message),
    }
}

fn encode_webp(
    engines: &EngineSet,
    input: &Path,
    kind: Kind,
    settings: &Settings,
    dest: &Path,
    cancel: &AtomicBool,
) -> Result<String, ToolRun> {
    if kind == Kind::Gif {
        let bin = engines.require("gif2webp").map_err(|e| ToolRun::Failed(e))?;
        let mut args = Vec::new();
        match settings.preset {
            Preset::Photo => {
                args.extend([
                    "-lossy".into(),
                    "-q".into(),
                    "85".into(),
                    "-m".into(),
                    "6".into(),
                ]);
            }
            Preset::Ui | Preset::Lossless => args.push("-lossless".into()),
        }
        args.extend([
            "-metadata".into(),
            "icc".into(),
            input.to_string_lossy().to_string(),
            "-o".into(),
            dest.to_string_lossy().to_string(),
        ]);
        match run_tool(&bin, &args, cancel, None) {
            ToolRun::Ok => Ok("gif2webp".into()),
            other => Err(other),
        }
    } else if kind == Kind::Svg {
        let png = dest.with_extension("png");
        rasterize_svg(input, &png).map_err(ToolRun::Failed)?;
        encode_webp(engines, &png, Kind::Png, settings, dest, cancel)
    } else {
        let bin = engines.require("cwebp").map_err(ToolRun::Failed)?;
        let mut args = match settings.preset {
            Preset::Photo => vec![
                "-q".into(),
                "85".into(),
                "-m".into(),
                "6".into(),
                "-sharp_yuv".into(),
            ],
            Preset::Ui => vec!["-near_lossless".into(), "60".into(), "-m".into(), "6".into()],
            Preset::Lossless => vec!["-lossless".into(), "-z".into(), "9".into()],
        };
        args.extend([
            "-metadata".into(),
            "icc".into(),
            "-alpha_q".into(),
            "100".into(),
            input.to_string_lossy().to_string(),
            "-o".into(),
            dest.to_string_lossy().to_string(),
        ]);
        match run_tool(&bin, &args, cancel, None) {
            ToolRun::Ok => Ok("cwebp".into()),
            other => Err(other),
        }
    }
}

fn encode_jpeg(
    engines: &EngineSet,
    input: &Path,
    kind: Kind,
    settings: &Settings,
    temp: &Path,
    dest: &Path,
    cancel: &AtomicBool,
) -> Result<String, ToolRun> {
    let png = if kind == Kind::Svg {
        let png = temp.join("flat.png");
        rasterize_svg(input, &png).map_err(ToolRun::Failed)?;
        png
    } else {
        input.to_path_buf()
    };
    let image = image::open(&png).map_err(|_| ToolRun::Failed("这张图无法读取".into()))?;
    let rgba = image.to_rgba8();
    let ppm = temp.join("flat.ppm");
    write_ppm(&ppm, &rgba, true).map_err(|_| ToolRun::Failed("无法写入临时文件".into()))?;
    let bin = engines.require("cjpeg").map_err(ToolRun::Failed)?;
    let quality = if settings.preset == Preset::Lossless { "100" } else { "85" };
    let bytes = run_tool_stdout(
        &bin,
        &[
            "-quality".into(),
            quality.into(),
            "-progressive".into(),
            "-optimize".into(),
            ppm.to_string_lossy().to_string(),
        ],
        cancel,
    )?;
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err(ToolRun::Failed("无法转换成 JPEG".into()));
    }
    fs::write(dest, bytes).map_err(|_| ToolRun::Failed("没有写入权限".into()))?;
    Ok(if settings.preset == Preset::Lossless {
        "cjpeg 最高质量".into()
    } else {
        "cjpeg".into()
    })
}

fn encode_png(
    engines: &EngineSet,
    input: &Path,
    kind: Kind,
    settings: &Settings,
    temp: &Path,
    dest: &Path,
    cancel: &AtomicBool,
) -> Result<String, ToolRun> {
    let raw = temp.join("raw.png");
    if kind == Kind::Svg {
        rasterize_svg(input, &raw).map_err(ToolRun::Failed)?;
    } else {
        let image = image::open(input).map_err(|_| ToolRun::Failed("这张图无法读取".into()))?;
        image
            .save(&raw)
            .map_err(|_| ToolRun::Failed("无法转换成 PNG".into()))?;
    }
    if settings.preset == Preset::Lossless {
        fs::copy(&raw, dest).map_err(|_| ToolRun::Failed("没有写入权限".into()))?;
        return Ok("png".into());
    }
    match engines.find("oxipng") {
        Some(bin) => {
            let args = vec![
                "-o".into(),
                "4".into(),
                "--strip".into(),
                "safe".into(),
                "--out".into(),
                dest.to_string_lossy().to_string(),
                raw.to_string_lossy().to_string(),
            ];
            match run_tool(&bin, &args, cancel, None) {
                ToolRun::Ok => Ok("oxipng".into()),
                ToolRun::Cancelled => Err(ToolRun::Cancelled),
                _ => {
                    fs::copy(&raw, dest).map_err(|_| ToolRun::Failed("没有写入权限".into()))?;
                    Ok("png".into())
                }
            }
        }
        None => {
            fs::copy(&raw, dest).map_err(|_| ToolRun::Failed("没有写入权限".into()))?;
            Ok("png".into())
        }
    }
}

fn extract_first_frame(input: &Path, temp: &Path) -> Result<PathBuf, &'static str> {
    let image = image::open(input).map_err(|_| "这张图无法读取")?;
    let dest = temp.join("frame.png");
    image.save(&dest).map_err(|_| "这张图无法读取")?;
    Ok(dest)
}

fn write_ppm(path: &Path, image: &image::RgbaImage, flatten: bool) -> std::io::Result<()> {
    let (width, height) = image.dimensions();
    let mut file = File::create(path)?;
    write!(file, "P6\n{width} {height}\n255\n")?;
    for pixel in image.pixels() {
        let (r, g, b) = if flatten {
            let alpha = u16::from(pixel[3]);
            let blend = |channel: u8| -> u8 {
                ((u16::from(channel) * alpha + 255 * (255 - alpha)) / 255) as u8
            };
            (blend(pixel[0]), blend(pixel[1]), blend(pixel[2]))
        } else {
            (pixel[0], pixel[1], pixel[2])
        };
        file.write_all(&[r, g, b])?;
    }
    Ok(())
}

fn publish(
    source: &Path,
    relative: &str,
    format: OutputFormat,
    kind: Kind,
    output_dir: &Path,
) -> Result<PathBuf, String> {
    let mut dest = safe_join(output_dir, relative)?;
    match format {
        OutputFormat::Original => {}
        OutputFormat::Webp => {
            dest.set_extension("webp");
        }
        OutputFormat::Jpeg => {
            dest.set_extension("jpg");
        }
        OutputFormat::Png => {
            dest.set_extension("png");
        }
    }
    if format == OutputFormat::Original && dest.extension().is_none() {
        dest.set_extension(kind.extension());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(io_message)?;
    }
    dest = unique_destination(dest);
    let tmp = dest.with_extension(format!(
        "{}.paretmp",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("bin")
    ));
    fs::copy(source, &tmp).map_err(io_message)?;
    fs::rename(&tmp, &dest).map_err(io_message)?;
    Ok(dest)
}

fn write_copy(
    input: &Path,
    relative: &str,
    kind: Kind,
    format: OutputFormat,
    input_bytes: u64,
    output_dir: &Path,
    note: &str,
) -> Outcome {
    match publish(input, relative, format, kind, output_dir) {
        Ok(dest) => Outcome {
            status: "unchanged",
            output_bytes: fs::metadata(&dest).ok().map(|m| m.len()).or(Some(input_bytes)),
            delta_ratio: Some(0.0),
            note: note.into(),
            output_path: Some(dest.to_string_lossy().to_string()),
        },
        Err(message) => failed(message),
    }
}

fn done(input_bytes: u64, dest: &Path, note: &str) -> Outcome {
    let output_bytes = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    Outcome {
        status: "done",
        output_bytes: Some(output_bytes),
        delta_ratio: ratio(input_bytes, output_bytes),
        note: note.into(),
        output_path: Some(dest.to_string_lossy().to_string()),
    }
}

fn ratio(input: u64, output: u64) -> Option<f64> {
    if input == 0 {
        return Some(0.0);
    }
    Some((output as f64 - input as f64) / input as f64)
}

fn smaller(candidate: &Path, current: &Path) -> bool {
    match (fs::metadata(candidate), fs::metadata(current)) {
        (Ok(a), Ok(b)) => a.len() > 0 && a.len() < b.len(),
        _ => false,
    }
}

fn blank(note: impl Into<String>) -> Outcome {
    Outcome {
        status: "failed",
        output_bytes: None,
        delta_ratio: None,
        note: note.into(),
        output_path: None,
    }
}

fn failed(message: impl Into<String>) -> Outcome {
    let mut outcome = blank(message);
    outcome.status = "failed";
    outcome
}

fn cancelled() -> Outcome {
    let mut outcome = blank("");
    outcome.status = "cancelled";
    outcome
}

fn io_message(err: std::io::Error) -> String {
    match err.raw_os_error() {
        Some(28) => "磁盘已满".into(),
        Some(13) | Some(1) => "没有写入权限".into(),
        _ if err.kind() == std::io::ErrorKind::PermissionDenied => "没有写入权限".into(),
        _ => "没有写入权限".into(),
    }
}

fn temp_dir() -> Result<PathBuf, &'static str> {
    let path = std::env::temp_dir().join(format!(
        "pare-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&path).map_err(|_| "无法写入临时文件")?;
    Ok(path)
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn jpeg_to_webp_uses_cwebp_when_present() {
        let engines = EngineSet;
        if engines.find("cwebp").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("pare-webp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("photo.jpg");
        let image = image::RgbImage::from_pixel(8, 8, image::Rgb([40, 90, 140]));
        image.save(&input).unwrap();
        let out_dir = dir.join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let outcome = process_file(
            &engines,
            &input,
            "photo.jpg",
            Kind::Jpeg,
            false,
            fs::metadata(&input).unwrap().len(),
            &Settings::default(),
            &out_dir,
            &AtomicBool::new(false),
        );
        assert_eq!(outcome.status, "done", "{}", outcome.note);
        assert!(out_dir.join("photo.webp").is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}
