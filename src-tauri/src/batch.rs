use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use tauri::{AppHandle, Emitter};

use crate::engines::{write_debug, EngineSet};
use crate::model::{BatchRequest, FileUpdate, JobRequest, Settings};
use crate::pipeline::process_file;
use crate::scan::output_nested_in_input;

struct QueueState {
    jobs: VecDeque<JobRequest>,
    active: usize,
    running: bool,
    finished: bool,
}

pub struct BatchControl {
    queue: Mutex<QueueState>,
    cv: Condvar,
    cancel: Arc<AtomicBool>,
    removed: Arc<Mutex<HashSet<String>>>,
    settings: Mutex<Settings>,
    output_dir: Mutex<PathBuf>,
    log_path: Mutex<Option<PathBuf>>,
}

impl BatchControl {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(QueueState {
                jobs: VecDeque::new(),
                active: 0,
                running: false,
                finished: false,
            }),
            cv: Condvar::new(),
            cancel: Arc::new(AtomicBool::new(false)),
            removed: Arc::new(Mutex::new(HashSet::new())),
            settings: Mutex::new(Settings::default()),
            output_dir: Mutex::new(PathBuf::new()),
            log_path: Mutex::new(None),
        }
    }
}

impl Default for BatchControl {
    fn default() -> Self {
        Self::new()
    }
}

pub fn start_batch(app: AppHandle, control: Arc<BatchControl>, request: BatchRequest) -> Result<(), String> {
    let settings = request.settings.normalized();
    let output = PathBuf::from(&request.output_dir);
    if request.output_dir.trim().is_empty() {
        return Err("请选择输出文件夹".into());
    }
    if output_nested_in_input(&output, &request.roots) {
        return Err("输出文件夹不能放在输入文件夹里面。".into());
    }
    std::fs::create_dir_all(&output).map_err(|_| "没有写入权限".to_string())?;
    let jobs: VecDeque<_> = request
        .jobs
        .into_iter()
        .filter(|job| job.kind != crate::model::Kind::Other)
        .collect();
    if jobs.is_empty() {
        return Err("没有可处理的图片".into());
    }
    {
        let mut queue = control.queue.lock().expect("queue");
        if queue.running {
            return Err("正在处理".into());
        }
        queue.jobs = jobs;
        queue.active = 0;
        queue.running = true;
        queue.finished = false;
        control.cancel.store(false, Ordering::Relaxed);
        control.removed.lock().expect("removed").clear();
        *control.settings.lock().expect("settings") = settings;
        *control.output_dir.lock().expect("output") = output;
    }
    let workers = control.settings.lock().expect("settings").concurrency.max(1);
    for _ in 0..workers {
        let app = app.clone();
        let control = control.clone();
        thread::spawn(move || worker(app, control));
    }
    control.cv.notify_all();
    Ok(())
}

pub fn enqueue(control: &BatchControl, jobs: Vec<JobRequest>) -> Result<(), String> {
    let mut queue = control.queue.lock().expect("queue");
    if !queue.running {
        return Err("not-running".into());
    }
    for job in jobs {
        if job.kind != crate::model::Kind::Other {
            queue.jobs.push_back(job);
        }
    }
    control.cv.notify_all();
    Ok(())
}

pub fn cancel_batch(control: &BatchControl) {
    control.cancel.store(true, Ordering::Relaxed);
    let mut queue = control.queue.lock().expect("queue");
    queue.jobs.clear();
    queue.running = false;
    control.cv.notify_all();
}

pub fn set_log(control: &BatchControl, path: Option<PathBuf>) {
    *control.log_path.lock().expect("log") = path;
}

pub fn remove_job(control: &BatchControl, id: &str) {
    control.removed.lock().expect("removed").insert(id.to_string());
    let mut queue = control.queue.lock().expect("queue");
    queue.jobs.retain(|job| job.id != id);
}

fn worker(app: AppHandle, control: Arc<BatchControl>) {
    let engines = EngineSet;
    loop {
        let job = {
            let mut queue = control.queue.lock().expect("queue");
            loop {
                if control.cancel.load(Ordering::Relaxed) && queue.jobs.is_empty() {
                    maybe_finish(&app, &control, &mut queue);
                    return;
                }
                if let Some(job) = queue.jobs.pop_front() {
                    if control.removed.lock().expect("removed").contains(&job.id) {
                        maybe_finish(&app, &control, &mut queue);
                        continue;
                    }
                    queue.active += 1;
                    break job;
                }
                if queue.active == 0 {
                    maybe_finish(&app, &control, &mut queue);
                    return;
                }
                queue = control.cv.wait(queue).expect("wait");
            }
        };
        let _ = app.emit(
            "file-updated",
            FileUpdate {
                id: job.id.clone(),
                status: "running".into(),
                output_bytes: None,
                delta_ratio: None,
                note: String::new(),
                output_path: None,
            },
        );
        let settings = control.settings.lock().expect("settings").clone();
        let output_dir = control.output_dir.lock().expect("output").clone();
        let outcome = process_file(
            &engines,
            PathBuf::from(&job.input_path).as_path(),
            &job.relative_path,
            job.kind,
            job.animated,
            job.input_bytes,
            &settings,
            &output_dir,
            &control.cancel,
        );
        if let Some(log) = control.log_path.lock().expect("log").clone() {
            write_debug(
                Some(log.as_path()),
                &format!("{} {} {}", job.input_path, outcome.status, outcome.note),
            );
        }
        let _ = app.emit(
            "file-updated",
            FileUpdate {
                id: job.id,
                status: outcome.status.into(),
                output_bytes: outcome.output_bytes,
                delta_ratio: outcome.delta_ratio,
                note: outcome.note,
                output_path: outcome.output_path,
            },
        );
        let mut queue = control.queue.lock().expect("queue");
        queue.active = queue.active.saturating_sub(1);
        maybe_finish(&app, &control, &mut queue);
        control.cv.notify_all();
    }
}

fn maybe_finish(app: &AppHandle, control: &BatchControl, queue: &mut QueueState) {
    if queue.finished {
        return;
    }
    if queue.jobs.is_empty() && queue.active == 0 {
        queue.running = false;
        queue.finished = true;
        let _ = app.emit("batch-finished", serde_json::json!({ "ok": true }));
        control.cv.notify_all();
    }
}
