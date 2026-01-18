use std::{
    io::{self, Read, Write},
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

// IMPORTANT: This module contains workarounds for tokio 1.36 compatibility
// Tokio 1.36 has issues with properly flushing async tasks on runtime shutdown,
// which causes responses to be dropped before being sent through stdio.
// In tokio 1.46+, the runtime handles this better.
//
// Known Issue: The tools/list response may not be sent before the service shuts down
// when stdin closes. This is a race condition in the rmcp library when used with
// tokio 1.36's runtime shutdown behavior.

pub struct StdinReader;
pub struct StdoutWriter;

impl AsyncRead for StdinReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut stdin = io::stdin();
        let mut temp = vec![0u8; buf.remaining()];
        match stdin.read(&mut temp) {
            Ok(n) => {
                buf.put_slice(&temp[..n]);
                Poll::Ready(Ok(()))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl AsyncWrite for StdoutWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut stdout = io::stdout();
        match stdout.write(buf) {
            Ok(n) => {
                // Force flush immediately after writing
                // This helps ensure data is sent even with tokio 1.36
                let _ = stdout.flush();
                Poll::Ready(Ok(n))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut stdout = io::stdout();
        match stdout.flush() {
            Ok(()) => Poll::Ready(Ok(())),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(cx)
    }
}

pub fn wasi_io() -> (StdinReader, StdoutWriter) {
    (StdinReader, StdoutWriter)
}
