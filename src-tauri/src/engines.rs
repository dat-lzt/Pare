use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::scan::imageoptim_engine_dir;

#[derive(Clone)]
pub struct EngineSet;

impl EngineSet {
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        let file_name = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        };
        for dir in candidate_dirs() {
            let path = dir.join(&file_name);
            if path.is_file() {
                return Some(path);
            }
        }
        None
    }

    pub fn require(&self, name: &str) -> Result<PathBuf, String> {
        self.find(name).ok_or_else(|| format!("缺少引擎 {name}"))
    }

    pub fn versions(&self) -> Vec<(String, String)> {
        [
            "cwebp",
            "gif2webp",
            "cjpeg",
            "oxipng",
            "pngquant",
            "zopflipng",
            "jpegtran",
            "jpegoptim",
            "gifsicle",
            "svgcleaner",
        ]
        .into_iter()
        .map(|name| {
            let text = match self.find(name) {
                Some(path) => version_of(&path).unwrap_or_else(|| name.to_string()),
                None => format!("{name} 未找到"),
            };
            (name.to_string(), text)
        })
        .collect()
    }
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(custom) = std::env::var_os("PARE_ENGINES") {
        dirs.push(PathBuf::from(custom));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("engines"));
            dirs.push(dir.join("../Resources/engines"));
        }
    }
    dirs.push(imageoptim_engine_dir().to_path_buf());
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs
}

fn version_of(path: &Path) -> Option<String> {
    for flag in ["-version", "--version", "-V"] {
        let output = Command::new(path)
            .arg(flag)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .ok()?;
        let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            text = String::from_utf8_lossy(&output.stderr).trim().to_string();
        }
        if let Some(line) = text.lines().next() {
            let line = line.trim();
            if !line.is_empty() && !line.to_ascii_lowercase().starts_with("usage") {
                return Some(line.to_string());
            }
        }
    }
    None
}

pub enum ToolRun {
    Ok,
    Failed(String),
    TimedOut,
    Cancelled,
}

pub fn run_tool(
    bin: &Path,
    args: &[String],
    cancel: &AtomicBool,
    timeout: Option<Duration>,
) -> ToolRun {
    if cancel.load(Ordering::Relaxed) {
        return ToolRun::Cancelled;
    }
    let mut command = Command::new(bin);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    lower_priority(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) => return ToolRun::Failed(err.to_string()),
    };
    let started = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return ToolRun::Cancelled;
        }
        if let Some(limit) = timeout {
            if started.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                return ToolRun::TimedOut;
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return ToolRun::Ok;
                }
                let mut err = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut err);
                }
                let err = err.trim();
                if err.is_empty() {
                    return ToolRun::Failed(format!("退出码 {}", status.code().unwrap_or(-1)));
                }
                return ToolRun::Failed(trim_note(err));
            }
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(err) => return ToolRun::Failed(err.to_string()),
        }
    }
}

pub fn run_tool_stdout(
    bin: &Path,
    args: &[String],
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ToolRun> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolRun::Cancelled);
    }
    let mut command = Command::new(bin);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    lower_priority(&mut command);
    let output = match command.output() {
        Ok(output) => output,
        Err(err) => return Err(ToolRun::Failed(err.to_string())),
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolRun::Cancelled);
    }
    if output.status.success() && !output.stdout.is_empty() {
        return Ok(output.stdout);
    }
    let err = String::from_utf8_lossy(&output.stderr);
    Err(ToolRun::Failed(trim_note(err.trim())))
}

fn trim_note(text: &str) -> String {
    let mut out = text.replace('\n', " ");
    if out.chars().count() > 180 {
        out = out.chars().take(180).collect();
    }
    out
}

fn lower_priority(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                setpriority(0, 0, 10);
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0000_4000);
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn setpriority(which: i32, who: u32, prio: i32) -> i32;
}

pub fn write_debug(log_path: Option<&Path>, line: &str) {
    let Some(path) = log_path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}
