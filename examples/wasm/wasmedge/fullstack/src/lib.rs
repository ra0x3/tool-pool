use tokio::io::{stdin, stdout, Stdin, Stdout};

mod server;
pub use server::*;

pub fn wasi_io() -> (Stdin, Stdout) {
    (stdin(), stdout())
}
