use super::parser::{extract_ready_segments, parse_exiftool_output, ExifToolResult};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

const COMMAND_TIMEOUT_MS: u64 = 30_000;
const ASSUMED_WRITE_FLOOR_BPS: u64 = 20_000_000;
const CLOSE_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Error)]
pub enum ExifToolError {
    #[error("The selected path contains an unsupported line break")]
    UnsafePath,
    #[error("ExifTool process is not open")]
    NotOpen,
    #[error("ExifTool command {execute_num} exceeded its {deadline_ms} ms deadline")]
    Timeout { execute_num: u64, deadline_ms: u64 },
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Protocol(String),
}

pub fn assert_safe_path(path: &str) -> Result<(), ExifToolError> {
    if path.contains('\r') || path.contains('\n') {
        Err(ExifToolError::UnsafePath)
    } else {
        Ok(())
    }
}

pub fn write_deadline_ms(source_bytes: u64) -> u64 {
    COMMAND_TIMEOUT_MS + (source_bytes.saturating_mul(1000) / ASSUMED_WRITE_FLOOR_BPS)
}

/// Resolve bundled resource first, then fall back to PATH.
pub fn resolve_exiftool_bin(resource_dir: Option<&Path>) -> PathBuf {
    if let Some(dir) = resource_dir {
        let candidates = [
            dir.join("bin").join("exiftool"),
            dir.join("bin").join("exiftool.exe"),
            dir.join("nix").join("bin").join("exiftool"),
            dir.join("win").join("bin").join("exiftool.exe"),
        ];
        for c in candidates {
            if c.exists() {
                return c;
            }
        }
    }

    if let Ok(path) = which_exiftool() {
        return path;
    }

    PathBuf::from(if cfg!(windows) {
        "exiftool.exe"
    } else {
        "exiftool"
    })
}

fn which_exiftool() -> Result<PathBuf, ()> {
    let (cmd, arg) = if cfg!(windows) {
        ("where", "exiftool")
    } else {
        ("which", "exiftool")
    };
    let output = std::process::Command::new(cmd)
        .arg(arg)
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if path.is_empty() {
        Err(())
    } else {
        Ok(PathBuf::from(path))
    }
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stdout_buffer: String,
    execute_counter: u64,
}

/// Single stay-open ExifTool session. One command in flight at a time (mutex).
pub struct ExiftoolProcess {
    bin_path: PathBuf,
    inner: Mutex<Option<Session>>,
}

impl ExiftoolProcess {
    pub fn new(bin_path: PathBuf) -> Self {
        Self {
            bin_path,
            inner: Mutex::new(None),
        }
    }

    pub async fn open(&self) -> Result<u32, ExifToolError> {
        let mut guard = self.inner.lock().await;
        if guard.is_some() {
            return Err(ExifToolError::Protocol(
                "ExifTool process is already open".into(),
            ));
        }
        let session = Self::spawn_session(&self.bin_path).await?;
        let pid = session.child.id().unwrap_or(0);
        *guard = Some(session);
        Ok(pid)
    }

    pub async fn close(&self) -> Result<(), ExifToolError> {
        let mut guard = self.inner.lock().await;
        let Some(mut session) = guard.take() else {
            return Ok(());
        };

        let _ = session.stdin.write_all(b"-stay_open\nFalse\n").await;
        let _ = session.stdin.shutdown().await;

        let started = Instant::now();
        loop {
            match session.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => {
                    if started.elapsed() > Duration::from_millis(CLOSE_TIMEOUT_MS) {
                        let _ = session.child.kill().await;
                        let _ = session.child.wait().await;
                        return Ok(());
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => return Err(ExifToolError::Io(e.to_string())),
            }
        }
    }

    pub async fn read_metadata(
        &self,
        file_path: &str,
        args: &[&str],
    ) -> Result<ExifToolResult, ExifToolError> {
        assert_safe_path(file_path)?;
        let mut lines: Vec<String> = vec!["-json".into()];
        lines.extend(args.iter().map(|s| (*s).to_string()));
        lines.push(file_path.to_string());
        self.run_command(lines, COMMAND_TIMEOUT_MS).await
    }

    pub async fn write_metadata(
        &self,
        file_path: &str,
        extra_args: &[String],
        deadline_ms: u64,
    ) -> Result<ExifToolResult, ExifToolError> {
        assert_safe_path(file_path)?;
        let mut lines = extra_args.to_vec();
        lines.push(file_path.to_string());
        self.run_command(lines, deadline_ms).await
    }

    async fn run_command(
        &self,
        mut lines: Vec<String>,
        deadline_ms: u64,
    ) -> Result<ExifToolResult, ExifToolError> {
        let mut guard = self.inner.lock().await;
        let session = guard.as_mut().ok_or(ExifToolError::NotOpen)?;

        let execute_num = session.execute_counter;
        session.execute_counter += 1;
        lines.push(format!("-execute{execute_num}"));
        let command = format!("{}\n", lines.join("\n"));

        if let Err(e) = session.stdin.write_all(command.as_bytes()).await {
            return Err(ExifToolError::Io(e.to_string()));
        }

        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        loop {
            if Instant::now() > deadline {
                // Kill and respawn so the next command gets a clean session.
                if let Some(mut dead) = guard.take() {
                    let _ = dead.child.kill().await;
                    let _ = dead.child.wait().await;
                }
                match Self::spawn_session(&self.bin_path).await {
                    Ok(session) => *guard = Some(session),
                    Err(_) => *guard = None,
                }
                return Err(ExifToolError::Timeout {
                    execute_num,
                    deadline_ms,
                });
            }

            let session = guard.as_mut().ok_or(ExifToolError::NotOpen)?;
            let mut line = String::new();
            match tokio::time::timeout(Duration::from_millis(200), session.stdout.read_line(&mut line))
                .await
            {
                Ok(Ok(0)) => {
                    return Err(ExifToolError::Io(
                        "ExifTool process closed unexpectedly".into(),
                    ));
                }
                Ok(Ok(_)) => {
                    session.stdout_buffer.push_str(&line);
                    let (segments, remaining) = extract_ready_segments(&session.stdout_buffer);
                    session.stdout_buffer = remaining;
                    for segment in segments {
                        if segment.execute_num == execute_num {
                            return Ok(parse_exiftool_output(&segment.output));
                        }
                    }
                }
                Ok(Err(e)) => return Err(ExifToolError::Io(e.to_string())),
                Err(_) => {
                    // read timed out — loop and check deadline
                }
            }
        }
    }

    async fn spawn_session(bin_path: &Path) -> Result<Session, ExifToolError> {
        let mut child = tokio::process::Command::new(bin_path)
            .args(["-stay_open", "True", "-@", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                ExifToolError::Io(format!(
                    "Failed to spawn ExifTool at {}: {e}",
                    bin_path.display()
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ExifToolError::Io("Missing ExifTool stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ExifToolError::Io("Missing ExifTool stdout".into()))?;

        Ok(Session {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            stdout_buffer: String::new(),
            execute_counter: 0,
        })
    }
}
