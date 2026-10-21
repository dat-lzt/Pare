use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputFormat {
    #[serde(rename = "original")]
    Original,
    #[serde(rename = "webp")]
    Webp,
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "png")]
    Png,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
    #[serde(rename = "photo")]
    Photo,
    #[serde(rename = "ui")]
    Ui,
    #[serde(rename = "lossless")]
    Lossless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[serde(rename = "jpeg")]
    Jpeg,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "gif")]
    Gif,
    #[serde(rename = "svg")]
    Svg,
    #[serde(rename = "other")]
    Other,
}

impl Kind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Self::Jpeg,
            "png" => Self::Png,
            "gif" => Self::Gif,
            "svg" => Self::Svg,
            _ => Self::Other,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Svg => "svg",
            Self::Other => "bin",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineToggles {
    pub oxipng: bool,
    pub pngquant: bool,
    pub zopflipng: bool,
    pub jpegtran: bool,
    pub jpegoptim: bool,
    pub gifsicle: bool,
    pub svgcleaner: bool,
}

impl Default for EngineToggles {
    fn default() -> Self {
        Self {
            oxipng: true,
            pngquant: true,
            zopflipng: true,
            jpegtran: true,
            jpegoptim: true,
            gifsicle: true,
            svgcleaner: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Default for Appearance {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en")]
    En,
}

impl Default for Language {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub output_format: OutputFormat,
    pub preset: Preset,
    pub lossy: bool,
    pub engines: EngineToggles,
    pub concurrency: u8,
    pub gif_first_frame: bool,
    pub debug_log: bool,
    #[serde(default)]
    pub appearance: Appearance,
    #[serde(default)]
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Webp,
            preset: Preset::Photo,
            lossy: false,
            engines: EngineToggles::default(),
            concurrency: 4,
            gif_first_frame: false,
            debug_log: false,
            appearance: Appearance::System,
            language: Language::System,
        }
    }
}

impl Settings {
    pub fn normalized(mut self) -> Self {
        self.concurrency = self.concurrency.clamp(1, 8);
        if self.preset == Preset::Lossless {
            self.lossy = false;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Unsupported,
    Fail(&'static str),
    Optimize(Kind),
    Transcode { to: OutputFormat, first_frame: bool },
}

pub fn decide(kind: Kind, animated: bool, settings: &Settings) -> Plan {
    if kind == Kind::Other {
        return Plan::Unsupported;
    }
    match settings.output_format {
        OutputFormat::Original => Plan::Optimize(kind),
        OutputFormat::Jpeg | OutputFormat::Png
            if kind == Kind::Gif && animated && !settings.gif_first_frame =>
        {
            Plan::Fail("动画不能变成单张静图")
        }
        OutputFormat::Jpeg | OutputFormat::Png if kind == Kind::Gif && animated => Plan::Transcode {
            to: settings.output_format,
            first_frame: true,
        },
        other => Plan::Transcode {
            to: other,
            first_frame: false,
        },
    }
}

pub fn gifsicle_lossy_level(quality: u8) -> u32 {
    (100 - u32::from(quality.min(100))) * 3 / 2
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub input_path: String,
    pub relative_path: String,
    pub input_bytes: u64,
    pub kind: Kind,
    pub animated: bool,
    pub has_alpha: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectResult {
    pub files: Vec<FileRecord>,
    pub roots: Vec<String>,
    pub suggested_output: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub id: String,
    pub input_path: String,
    pub relative_path: String,
    pub kind: Kind,
    pub animated: bool,
    pub input_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUpdate {
    pub id: String,
    pub status: String,
    pub output_bytes: Option<u64>,
    pub delta_ratio: Option<f64>,
    pub note: String,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    pub jobs: Vec<JobRequest>,
    pub settings: Settings,
    pub output_dir: String,
    pub roots: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(format: OutputFormat, preset: Preset) -> Settings {
        Settings {
            output_format: format,
            preset,
            ..Settings::default()
        }
    }

    #[test]
    fn original_jpeg_optimizes() {
        let plan = decide(Kind::Jpeg, false, &settings(OutputFormat::Original, Preset::Photo));
        assert_eq!(plan, Plan::Optimize(Kind::Jpeg));
    }

    #[test]
    fn webp_transcodes_once() {
        let plan = decide(Kind::Jpeg, false, &settings(OutputFormat::Webp, Preset::Photo));
        assert_eq!(
            plan,
            Plan::Transcode {
                to: OutputFormat::Webp,
                first_frame: false
            }
        );
    }

    #[test]
    fn animated_gif_refuses_jpeg_until_first_frame() {
        let mut s = settings(OutputFormat::Jpeg, Preset::Photo);
        assert_eq!(
            decide(Kind::Gif, true, &s),
            Plan::Fail("动画不能变成单张静图")
        );
        s.gif_first_frame = true;
        assert_eq!(
            decide(Kind::Gif, true, &s),
            Plan::Transcode {
                to: OutputFormat::Jpeg,
                first_frame: true
            }
        );
    }

    #[test]
    fn lossless_jpeg_stays_jpeg() {
        let s = settings(OutputFormat::Jpeg, Preset::Lossless).normalized();
        assert_eq!(s.output_format, OutputFormat::Jpeg);
        assert!(!s.lossy);
        assert_eq!(
            decide(Kind::Png, false, &s),
            Plan::Transcode {
                to: OutputFormat::Jpeg,
                first_frame: false
            }
        );
    }

    #[test]
    fn lossy_gif_level_at_quality_80() {
        assert_eq!(gifsicle_lossy_level(80), 30);
        assert_eq!(gifsicle_lossy_level(100), 0);
    }
}
