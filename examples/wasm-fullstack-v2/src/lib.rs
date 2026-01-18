use tokio::io::{stdin, stdout, Stdin, Stdout};

// For WasmEdge compatibility, we use tokio's stdin/stdout directly
pub fn wasi_io() -> (Stdin, Stdout) {
    (stdin(), stdout())
}
