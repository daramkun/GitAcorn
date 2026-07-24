//! System Git infrastructure shared by application use cases.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{Error as IoError, Read};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

const REDACTED: &str = "***";

#[derive(Debug, Clone)]
pub struct GitRequest {
    pub working_directory: Option<PathBuf>,
    pub args: Vec<OsString>,
    pub timeout: Duration,
    pub environment: BTreeMap<OsString, OsString>,
}

impl GitRequest {
    pub fn new(args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        Self {
            working_directory: None,
            args: args.into_iter().map(Into::into).collect(),
            timeout: Duration::from_secs(10),
            environment: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub duration: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Error)]
pub enum GitExecutionError {
    #[error("Git executable was not found")]
    ExecutableNotFound,
    #[error("Git operation timed out")]
    TimedOut,
    #[error("Git operation was cancelled")]
    Cancelled,
    #[error("Could not execute Git: {0}")]
    Io(#[source] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct GitExecutor {
    executable: OsString,
}

impl Default for GitExecutor {
    fn default() -> Self {
        Self::new("git")
    }
}

impl GitExecutor {
    pub fn new(executable: impl Into<OsString>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    pub fn execute(
        &self,
        request: GitRequest,
        cancellation: &CancellationToken,
    ) -> Result<GitOutput, GitExecutionError> {
        let started = Instant::now();
        let mut command = Command::new(&self.executable);
        command
            .args(&request.args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(directory) = request.working_directory {
            command.current_dir(directory);
        }
        for (key, value) in request.environment {
            command.env(key, value);
        }
        hide_console_window(&mut command);

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GitExecutionError::ExecutableNotFound
            } else {
                GitExecutionError::Io(error)
            }
        })?;
        let stdout_reader = read_stream(child.stdout.take().expect("stdout is piped"));
        let stderr_reader = read_stream(child.stderr.take().expect("stderr is piped"));

        let completion = loop {
            if cancellation.is_cancelled() {
                break Err(GitExecutionError::Cancelled);
            }
            if started.elapsed() >= request.timeout {
                break Err(GitExecutionError::TimedOut);
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => break Err(GitExecutionError::Io(error)),
            }
        };

        if completion.is_err() {
            terminate(&mut child);
        }
        let stdout = join_reader(stdout_reader)?;
        let stderr = join_reader(stderr_reader)?;
        let status = completion?;
        Ok(GitOutput {
            stdout,
            stderr,
            exit_code: status.code().unwrap_or(-1),
            duration: started.elapsed(),
        })
    }

    pub fn executable(&self) -> &OsStr {
        &self.executable
    }
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn read_stream(
    mut stream: impl Read + Send + 'static,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        stream.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, GitExecutionError> {
    reader
        .join()
        .map_err(|_| GitExecutionError::Io(IoError::other("Git output reader panicked")))?
        .map_err(GitExecutionError::Io)
}

#[cfg(windows)]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_command: &mut Command) {}

/// Removes user information from HTTP(S) remote URLs before diagnostics are emitted.
pub fn redact_remote(remote: &str) -> String {
    let Some(scheme_end) = remote.find("://") else {
        return remote.to_owned();
    };
    let authority_start = scheme_end + 3;
    let Some(relative_at) = remote[authority_start..].find('@') else {
        return remote.to_owned();
    };
    let at = authority_start + relative_at;

    format!(
        "{}{}{}",
        &remote[..authority_start],
        REDACTED,
        &remote[at..]
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::time::Duration;

    use super::{CancellationToken, GitExecutor, GitRequest, redact_remote};

    #[test]
    fn masks_credentials_in_https_remote() {
        assert_eq!(
            redact_remote("https://alice:secret@example.com/org/repo.git"),
            "https://***@example.com/org/repo.git"
        );
    }

    #[test]
    fn preserves_remote_without_inline_credentials() {
        assert_eq!(
            redact_remote("https://example.com/org/repo.git"),
            "https://example.com/org/repo.git"
        );
    }

    #[test]
    fn executes_git_without_a_shell() {
        let executor = GitExecutor::default();
        let output = executor
            .execute(
                GitRequest::new([OsString::from("--version")]),
                &CancellationToken::default(),
            )
            .expect("Git is installed for tests");

        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.starts_with(b"git version "));
    }

    #[test]
    fn cancellation_token_is_observable() {
        let token = CancellationToken::default();
        token.cancel();

        assert!(token.is_cancelled());
    }

    #[test]
    fn request_has_a_safe_default_timeout() {
        let request = GitRequest::new(["status"]);
        assert_eq!(request.timeout, Duration::from_secs(10));
    }
}
