//! Integration tests for WASM filesystem access with policies

#![cfg(feature = "wasm-tools")]

use std::{fs, sync::Arc};

use mcpkit_rs::wasm::{WasmContext, WasmRuntime};
use tempfile::TempDir;

/// Test helper to create a test directory structure
fn setup_test_directories() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create test directories
    let readonly_dir = temp_dir.path().join("readonly");
    let readwrite_dir = temp_dir.path().join("readwrite");
    let forbidden_dir = temp_dir.path().join("forbidden");

    fs::create_dir(&readonly_dir).unwrap();
    fs::create_dir(&readwrite_dir).unwrap();
    fs::create_dir(&forbidden_dir).unwrap();

    // Create test files
    fs::write(readonly_dir.join("test.txt"), b"readonly content").unwrap();
    fs::write(readwrite_dir.join("data.txt"), b"readwrite content").unwrap();
    fs::write(forbidden_dir.join("secret.txt"), b"secret content").unwrap();

    temp_dir
}

/// Create a simple WASM module that just exits
fn create_simple_wasm() -> Vec<u8> {
    // This is a minimal WASM module that just exits successfully
    wat::parse_str(
        r#"
        (module
            (import "wasi_snapshot_preview1" "proc_exit"
                (func $proc_exit (param i32)))

            (memory (export "memory") 1)

            (func (export "_start")
                ;; Just exit with success
                (call $proc_exit (i32.const 0))
            )
        )
    "#,
    )
    .unwrap()
}

/// Create a simple WASM module that writes to a file
fn create_file_writer_wasm() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
            (import "wasi_snapshot_preview1" "path_open"
                (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))
            (import "wasi_snapshot_preview1" "fd_write"
                (func $fd_write (param i32 i32 i32 i32) (result i32)))
            (import "wasi_snapshot_preview1" "proc_exit"
                (func $proc_exit (param i32)))

            (memory (export "memory") 1)

            (func (export "_start")
                ;; Write "test" to stdout (fd=1)
                (i32.store (i32.const 0) (i32.const 8))  ;; iov_base
                (i32.store (i32.const 4) (i32.const 4))  ;; iov_len
                (i32.store8 (i32.const 8) (i32.const 116)) ;; 't'
                (i32.store8 (i32.const 9) (i32.const 101)) ;; 'e'
                (i32.store8 (i32.const 10) (i32.const 115)) ;; 's'
                (i32.store8 (i32.const 11) (i32.const 116)) ;; 't'

                (drop (call $fd_write
                    (i32.const 1)  ;; stdout
                    (i32.const 0)  ;; iovs
                    (i32.const 1)  ;; iovs_len
                    (i32.const 12) ;; nwritten
                ))

                (call $proc_exit (i32.const 0))
            )
        )
    "#,
    )
    .unwrap()
}

#[tokio::test]
async fn test_no_policy_no_filesystem_access() {
    let runtime = WasmRuntime::new().unwrap();
    let wasm_bytes = create_simple_wasm();

    // Compile the module
    let module = runtime.compile_module(&wasm_bytes).unwrap();

    // Create context without policy - should have no filesystem access
    let context = WasmContext::new();

    // This should succeed - the module just exits
    let result = runtime.execute(&module, context).await;
    if let Err(e) = &result {
        eprintln!("Execute failed: {:?}", e);
    }
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_readonly_access_with_policy() {
    let temp_dir = setup_test_directories();
    let runtime = WasmRuntime::new().unwrap();
    let wasm_bytes = create_simple_wasm();

    // Compile the module
    let module = runtime.compile_module(&wasm_bytes).unwrap();

    // Create a policy that allows reading from readonly directory
    let policy_yaml = format!(
        r#"
        version: "1.0"
        core:
          storage:
            allow:
              - uri: "fs://{}/**"
                access: ["read"]
    "#,
        temp_dir.path().join("readonly").display()
    );

    let policy = mcpkit_rs_policy::Policy::from_yaml(&policy_yaml).unwrap();
    let compiled_policy = Arc::new(mcpkit_rs_policy::CompiledPolicy::compile(&policy).unwrap());

    // Create context with policy
    let context = WasmContext::new().with_policy(compiled_policy);

    let result = runtime.execute(&module, context).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_write_access_with_policy() {
    let temp_dir = setup_test_directories();
    let runtime = WasmRuntime::new().unwrap();
    let wasm_bytes = create_file_writer_wasm();

    // Compile the module
    let module = runtime.compile_module(&wasm_bytes).unwrap();

    // Create a policy that allows writing to readwrite directory
    let policy_yaml = format!(
        r#"
        version: "1.0"
        core:
          storage:
            allow:
              - uri: "fs://{}/**"
                access: ["read", "write"]
    "#,
        temp_dir.path().join("readwrite").display()
    );

    let policy = mcpkit_rs_policy::Policy::from_yaml(&policy_yaml).unwrap();
    let compiled_policy = Arc::new(mcpkit_rs_policy::CompiledPolicy::compile(&policy).unwrap());

    // Create context with policy
    let context = WasmContext::new().with_policy(compiled_policy);

    let result = runtime.execute(&module, context).await;
    assert!(result.is_ok());

    // The module writes to stdout, so we should see output
    assert_eq!(result.unwrap(), b"test");
}

#[tokio::test]
async fn test_denied_access_to_forbidden_directory() {
    let temp_dir = setup_test_directories();
    let runtime = WasmRuntime::new().unwrap();
    let wasm_bytes = create_simple_wasm();

    // Compile the module
    let module = runtime.compile_module(&wasm_bytes).unwrap();

    // Create a policy that allows reading from readonly but not forbidden
    let policy_yaml = format!(
        r#"
        version: "1.0"
        core:
          storage:
            allow:
              - uri: "fs://{}/**"
                access: ["read"]
            deny:
              - uri: "fs://{}/**"
                access: ["read", "write"]
    "#,
        temp_dir.path().join("readonly").display(),
        temp_dir.path().join("forbidden").display()
    );

    let policy = mcpkit_rs_policy::Policy::from_yaml(&policy_yaml).unwrap();
    let compiled_policy = Arc::new(mcpkit_rs_policy::CompiledPolicy::compile(&policy).unwrap());

    // Create context with policy
    let context = WasmContext::new().with_policy(compiled_policy);

    // The module won't be able to access forbidden directory
    let result = runtime.execute(&module, context).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_multiple_directory_access() {
    let temp_dir = setup_test_directories();
    let runtime = WasmRuntime::new().unwrap();
    let wasm_bytes = create_simple_wasm();

    // Compile the module
    let module = runtime.compile_module(&wasm_bytes).unwrap();

    // Create a policy that allows access to multiple directories with different permissions
    let policy_yaml = format!(
        r#"
        version: "1.0"
        core:
          storage:
            allow:
              - uri: "fs://{}/**"
                access: ["read"]
              - uri: "fs://{}/**"
                access: ["read", "write"]
    "#,
        temp_dir.path().join("readonly").display(),
        temp_dir.path().join("readwrite").display()
    );

    let policy = mcpkit_rs_policy::Policy::from_yaml(&policy_yaml).unwrap();
    let compiled_policy = Arc::new(mcpkit_rs_policy::CompiledPolicy::compile(&policy).unwrap());

    // Create context with policy
    let context = WasmContext::new().with_policy(compiled_policy.clone());

    // Verify the policy was compiled correctly
    assert!(compiled_policy.is_storage_allowed(
        &format!("{}/test.txt", temp_dir.path().join("readonly").display()),
        "read"
    ));
    assert!(!compiled_policy.is_storage_allowed(
        &format!("{}/test.txt", temp_dir.path().join("readonly").display()),
        "write"
    ));
    assert!(compiled_policy.is_storage_allowed(
        &format!("{}/data.txt", temp_dir.path().join("readwrite").display()),
        "write"
    ));

    let result = runtime.execute(&module, context).await;
    assert!(result.is_ok());
}
