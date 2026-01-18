use std::process::Stdio;

use futures::future::Future;
use process_wrap::tokio::{TokioChildWrapper, TokioCommandWrap};
use tokio::{
    io::AsyncRead,
    process::{ChildStderr, ChildStdin, ChildStdout},
};

use super::{RxJsonRpcMessage, Transport, TxJsonRpcMessage, async_rw::AsyncRwTransport};
use crate::RoleClient;

/// The parts of a child process.
type ChildProcessParts = (
    Box<dyn TokioChildWrapper>,
    ChildStdout,
    ChildStdin,
    Option<ChildStderr>,
);

/// Extract the stdio handles from a spawned child.
/// Returns `(child, stdout, stdin, stderr)` where `stderr` is `Some` only
/// if the process was spawned with `Stdio::piped()`.
#[inline]
fn child_process(mut child: Box<dyn TokioChildWrapper>) -> std::io::Result<ChildProcessParts> {
    let child_stdin = match child.inner_mut().stdin().take() {
        Some(stdin) => stdin,
        None => return Err(std::io::Error::other("stdin was already taken")),
    };
    let child_stdout = match child.inner_mut().stdout().take() {
        Some(stdout) => stdout,
        None => return Err(std::io::Error::other("stdout was already taken")),
    };
    let child_stderr = child.inner_mut().stderr().take();
    Ok((child, child_stdout, child_stdin, child_stderr))
}

pub struct TokioChildProcess {
    child: ChildWithCleanup,
    transport: AsyncRwTransport<RoleClient, ChildStdout, ChildStdin>,
}

pub struct ChildWithCleanup {
    inner: Option<Box<dyn TokioChildWrapper>>,
}

impl Drop for ChildWithCleanup {
    fn drop(&mut self) {
        // We should not use start_kill(), instead we should use kill() to avoid zombies
        if let Some(mut inner) = self.inner.take() {
            // In Drop, we can't use async, so we use start_kill() which is sync
            // This is a best-effort attempt to kill the process
            if let Err(e) = inner.inner_mut().start_kill() {
                tracing::warn!("Error killing child process: {}", e);
            }
        }
    }
}

// we hold the child process with stdout, for it's easier to implement AsyncRead
pin_project_lite::pin_project! {
    pub struct TokioChildProcessOut {
        child: ChildWithCleanup,
        #[pin]
        child_stdout: ChildStdout,
    }
}

impl TokioChildProcessOut {
    /// Get the process ID of the child process.
    pub fn id(&self) -> Option<u32> {
        self.child.inner.as_ref()?.id()
    }
}

impl AsyncRead for TokioChildProcessOut {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().child_stdout.poll_read(cx, buf)
    }
}

impl TokioChildProcess {
    /// Convenience: spawn with default `piped` stdio
    pub fn new(command: tokio::process::Command) -> std::io::Result<Self> {
        let (proc, _ignored) = TokioChildProcessBuilder::new(command).spawn()?;
        Ok(proc)
    }

    /// Builder entry-point allowing fine-grained stdio control.
    pub fn builder(command: tokio::process::Command) -> TokioChildProcessBuilder {
        TokioChildProcessBuilder::new(command)
    }

    /// Get the process ID of the child process.
    pub fn id(&self) -> Option<u32> {
        self.child.inner.as_ref()?.id()
    }

    /// Gracefully shutdown the child process
    ///
    /// This will first close the transport to the child process (the server),
    /// and wait for the child process to exit normally with a timeout.
    /// If the child process doesn't exit within the timeout, it will be killed.
    pub async fn graceful_shutdown(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.child.inner.take() {
            self.transport.close().await?;

            // Give the child process a chance to exit gracefully
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Check if process has already exited
            match child.inner_mut().try_wait() {
                Ok(Some(status)) => {
                    tracing::info!("Child exited gracefully {}", status);
                    return Ok(());
                }
                Ok(None) => {
                    // Process is still running, kill it
                    tracing::info!("Child still running, killing it");
                }
                Err(e) => {
                    tracing::warn!("Error checking child status: {e}");
                }
            }

            // Kill the process synchronously
            if let Err(e) = child.inner_mut().start_kill() {
                tracing::warn!("Error killing child: {e}");
                // Don't return error as the process might have already exited
            }

            // Give it a bit more time to actually die
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            // Final cleanup attempt
            match child.inner_mut().try_wait() {
                Ok(Some(status)) => {
                    tracing::info!("Child killed successfully: {}", status);
                }
                Ok(None) => {
                    tracing::warn!("Child process may still be running");
                }
                Err(e) => {
                    tracing::warn!("Error in final wait: {e}");
                }
            }
        }
        Ok(())
    }

    /// Take ownership of the inner child process
    pub fn into_inner(mut self) -> Option<Box<dyn TokioChildWrapper>> {
        self.child.inner.take()
    }

    /// Split this helper into a reader (stdout) and writer (stdin).
    #[deprecated(
        since = "0.5.0",
        note = "use the Transport trait implementation instead"
    )]
    pub fn split(self) -> (TokioChildProcessOut, ChildStdin) {
        unimplemented!("This method is deprecated, use the Transport trait implementation instead");
    }
}

/// Builder for `TokioChildProcess` allowing custom `Stdio` configuration.
pub struct TokioChildProcessBuilder {
    cmd: tokio::process::Command,
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,
}

impl TokioChildProcessBuilder {
    fn new(cmd: tokio::process::Command) -> Self {
        Self {
            cmd,
            stdin: Stdio::piped(),
            stdout: Stdio::piped(),
            stderr: Stdio::inherit(),
        }
    }

    /// Override the child stdin configuration.
    pub fn stdin(mut self, io: impl Into<Stdio>) -> Self {
        self.stdin = io.into();
        self
    }
    /// Override the child stdout configuration.
    pub fn stdout(mut self, io: impl Into<Stdio>) -> Self {
        self.stdout = io.into();
        self
    }
    /// Override the child stderr configuration.
    pub fn stderr(mut self, io: impl Into<Stdio>) -> Self {
        self.stderr = io.into();
        self
    }

    /// Spawn the child process. Returns the transport plus an optional captured stderr handle.
    pub fn spawn(mut self) -> std::io::Result<(TokioChildProcess, Option<ChildStderr>)> {
        // Configure stdio on the command
        self.cmd
            .stdin(self.stdin)
            .stdout(self.stdout)
            .stderr(self.stderr);

        // Convert to TokioCommandWrap and spawn
        let mut wrapped_cmd: TokioCommandWrap = self.cmd.into();
        let (child, stdout, stdin, stderr_opt) = child_process(wrapped_cmd.spawn()?)?;

        let transport = AsyncRwTransport::new(stdout, stdin);
        let proc = TokioChildProcess {
            child: ChildWithCleanup { inner: Some(child) },
            transport,
        };
        Ok((proc, stderr_opt))
    }
}

impl Transport<RoleClient> for TokioChildProcess {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.transport.send(item)
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.transport.receive()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.graceful_shutdown()
    }
}

pub trait ConfigureCommandExt {
    fn configure(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl ConfigureCommandExt for tokio::process::Command {
    fn configure(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use tokio::process::Command;

    use super::*;

    const MAX_WAIT_ON_DROP_SECS: u64 = 3;

    #[tokio::test]
    async fn test_tokio_child_process_drop() {
        let r = TokioChildProcess::new(Command::new("sleep").configure(|cmd| {
            cmd.arg("30");
        }));
        assert!(r.is_ok());
        let child_process = r.unwrap();
        let id = child_process.id();
        assert!(id.is_some());
        let id = id.unwrap();
        // Drop the child process
        drop(child_process);
        // Wait a moment to allow the cleanup task to run
        tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS + 1)).await;
        // Check if the process is still running
        let status = Command::new("ps")
            .arg("-p")
            .arg(id.to_string())
            .status()
            .await;
        match status {
            Ok(status) => {
                assert!(
                    !status.success(),
                    "Process with PID {} is still running",
                    id
                );
            }
            Err(e) => {
                panic!("Failed to check process status: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_tokio_child_process_graceful_shutdown() {
        let r = TokioChildProcess::new(Command::new("sleep").configure(|cmd| {
            cmd.arg("30");
        }));
        assert!(r.is_ok());
        let mut child_process = r.unwrap();
        let id = child_process.id();
        assert!(id.is_some());
        let id = id.unwrap();
        child_process.graceful_shutdown().await.unwrap();
        // Wait a moment to allow the cleanup task to run
        tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS + 1)).await;
        // Check if the process is still running
        let status = Command::new("ps")
            .arg("-p")
            .arg(id.to_string())
            .status()
            .await;
        match status {
            Ok(status) => {
                assert!(
                    !status.success(),
                    "Process with PID {} is still running",
                    id
                );
            }
            Err(e) => {
                panic!("Failed to check process status: {}", e);
            }
        }
    }
}
