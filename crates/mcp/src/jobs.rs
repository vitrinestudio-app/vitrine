//! Background exports for MCP clients.
//!
//! A full export with large photos can take longer than an MCP client
//! waits for a tool call (often 60–80 s). The `export` tool therefore runs
//! the export on its own thread and waits at most `wait_seconds` for it;
//! when the export is still running, the call returns its job id and
//! progress, and `export_status` reports progress and – once finished – the
//! same result `export` would have returned. One export runs at a time
//! (the app refuses a second one anyway).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;
use vitrine_app::{CommandError, ErrorCode};

use crate::output::ToolOutput;
use crate::progress::ProgressRelay;

/// Result of a finished export (the `export` tool's output).
pub type JobResult = Result<ToolOutput, CommandError>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The exports started over MCP (the last one is kept for its result).
#[derive(Debug, Default)]
pub struct ExportJobs {
    current: Mutex<Option<Arc<ExportJob>>>,
    next: AtomicU64,
}

/// One export running (or finished) on its own thread.
pub struct ExportJob {
    pub id: String,
    started: Instant,
    state: Mutex<JobState>,
    finished: Condvar,
    /// `export-progress` payloads (`{file, done, total}`) while running.
    progress: Mutex<Option<UnboundedReceiver<Value>>>,
}

impl std::fmt::Debug for ExportJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportJob")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct JobState {
    done: u64,
    total: u64,
    last_file: Option<String>,
    result: Option<JobResult>,
    elapsed: Option<Duration>,
}

impl ExportJobs {
    /// Starts `run` on a new thread as the current export job. Fails with
    /// `busy` while the previous job still runs.
    pub fn start(
        &self,
        relay: Option<&Arc<ProgressRelay>>,
        run: impl FnOnce() -> JobResult + Send + 'static,
    ) -> Result<Arc<ExportJob>, CommandError> {
        let mut current = lock(&self.current);
        if let Some(job) = current.as_ref().filter(|j| !j.is_finished()) {
            return Err(CommandError::new(
                ErrorCode::Busy,
                format!(
                    "export `{}` is still running: wait for it with export_status {{job: \"{}\"}}",
                    job.id, job.id
                ),
            ));
        }
        let id = format!("export-{}", self.next.fetch_add(1, Ordering::Relaxed) + 1);
        let job = Arc::new(ExportJob {
            id,
            started: Instant::now(),
            state: Mutex::new(JobState::default()),
            finished: Condvar::new(),
            progress: Mutex::new(relay.map(|r| r.subscribe())),
        });
        *current = Some(job.clone());
        drop(current);
        let worker = job.clone();
        std::thread::Builder::new()
            .name("mcp-export".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|_| {
                    Err(CommandError::internal("the export panicked".to_owned()))
                });
                worker.finish(result);
            })
            .map_err(|e| CommandError::internal(format!("cannot start the export: {e}")))?;
        Ok(job)
    }

    /// The job `id` (`None` = the last one started).
    pub fn get(&self, id: Option<&str>) -> Option<Arc<ExportJob>> {
        lock(&self.current)
            .as_ref()
            .filter(|j| id.is_none_or(|id| j.id == id))
            .cloned()
    }
}

impl ExportJob {
    pub fn is_finished(&self) -> bool {
        lock(&self.state).result.is_some()
    }

    fn finish(&self, result: JobResult) {
        self.drain();
        // Stop listening: the relay drops the closed channel.
        lock(&self.progress).take();
        let mut state = lock(&self.state);
        state.result = Some(result);
        state.elapsed = Some(self.started.elapsed());
        drop(state);
        self.finished.notify_all();
    }

    /// Waits up to `timeout` for the result (`None`: still running).
    pub fn wait(&self, timeout: Duration) -> Option<JobResult> {
        let state = lock(&self.state);
        let (state, _) = self
            .finished
            .wait_timeout_while(state, timeout, |s| s.result.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        state.result.clone()
    }

    /// Takes the progress reports received so far.
    fn drain(&self) {
        let mut rx = lock(&self.progress);
        let Some(rx) = rx.as_mut() else { return };
        let mut state = lock(&self.state);
        while let Ok(v) = rx.try_recv() {
            state.done = v["done"].as_u64().unwrap_or(state.done);
            state.total = v["total"].as_u64().unwrap_or(state.total);
            if let Some(p) = v["file"]["path"].as_str() {
                state.last_file = Some(p.to_owned());
            }
        }
    }

    /// Progress of a running job: `{job, status: "running", done, total,
    /// last_file, elapsed_s}`.
    pub fn progress(&self) -> Value {
        self.drain();
        let state = lock(&self.state);
        let elapsed = state.elapsed.unwrap_or_else(|| self.started.elapsed());
        json!({
            "job": self.id,
            "status": if state.result.is_some() { "finished" } else { "running" },
            "done": state.done,
            "total": state.total,
            "last_file": state.last_file,
            "elapsed_s": (elapsed.as_secs_f64() * 10.0).round() / 10.0,
        })
    }

    /// The tool output of a job that is still running.
    pub fn running_output(&self) -> ToolOutput {
        let p = self.progress();
        let total = p["total"].as_u64().unwrap_or(0);
        let line = format!(
            "export `{}` is still running ({}{} file(s) done after {} s): call export_status {{job: \"{}\"}} until it is finished; the files appear in the export folder as they are written",
            self.id,
            p["done"],
            if total > 0 {
                format!(" of {total}")
            } else {
                String::new()
            },
            p["elapsed_s"],
            self.id
        );
        ToolOutput::new(line, p)
    }
}

/// The result of a finished job with its id added.
pub fn finished_output(job: &ExportJob, result: JobResult) -> JobResult {
    result.map(|mut out| {
        if let Value::Object(m) = &mut out.data {
            m.insert("job".into(), json!(job.id));
            m.insert("status".into(), json!("finished"));
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_early_and_reports_the_result_later() {
        let jobs = ExportJobs::default();
        let relay = Arc::new(ProgressRelay::default());
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let emitter = relay.clone();
        let job = jobs
            .start(Some(&relay), move || {
                use vitrine_app::EventSink;
                emitter.emit(
                    vitrine_app::events::EXPORT_PROGRESS,
                    json!({"done": 1, "total": 3, "file": {"path": "a/en/01.png"}}),
                );
                rx.recv().ok();
                Ok(ToolOutput::new("exported", json!({"files": []})))
            })
            .unwrap();
        assert!(job.wait(Duration::from_millis(50)).is_none());
        // A second export is refused while the first runs.
        let busy = jobs.start(None, || Ok(ToolOutput::default())).unwrap_err();
        assert_eq!(busy.code, ErrorCode::Busy);
        // Progress arrives through the relay.
        let deadline = Instant::now() + Duration::from_secs(5);
        while job.progress()["done"] != 1 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        let p = job.progress();
        assert_eq!(
            (p["done"].as_u64(), p["total"].as_u64()),
            (Some(1), Some(3))
        );
        assert_eq!(p["last_file"], "a/en/01.png");
        assert_eq!(p["status"], "running");
        tx.send(()).unwrap();
        let r = job.wait(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(r.summary, "exported");
        let r = finished_output(&job, Ok(r)).unwrap();
        assert_eq!(r.data["job"], "export-1");
        assert_eq!(jobs.get(None).unwrap().id, "export-1");
        assert!(jobs.get(Some("export-9")).is_none());
        // Finished: a new export may start.
        let next = jobs.start(None, || Ok(ToolOutput::default())).unwrap();
        assert_eq!(next.id, "export-2");
    }

    #[test]
    fn a_panicking_export_is_an_error() {
        let jobs = ExportJobs::default();
        let job = jobs.start(None, || panic!("boom")).unwrap();
        let r = job.wait(Duration::from_secs(5)).unwrap();
        assert!(r.unwrap_err().message.contains("panicked"));
    }
}
