//! Integration tests for WASM tools with actual compiled modules

#![cfg(feature = "wasm-tools")]

use std::{path::PathBuf, sync::Arc};

use rmcp::{
    model::CallToolResult,
    wasm::{WasmToolExecutor, WasmToolRegistry, credentials::InMemoryCredentialProvider},
};
use serde_json::json;

/// Build and prepare the calculator WASM module for testing
async fn ensure_calculator_wasm() -> PathBuf {
    let calculator_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-wasm");

    // Build the WASM module
    let output = tokio::process::Command::new("cargo")
        .arg("build")
        .arg("--target")
        .arg("wasm32-wasip1")
        .arg("--release")
        .current_dir(&calculator_dir)
        .output()
        .await
        .expect("Failed to build calculator WASM");

    if !output.status.success() {
        panic!(
            "Failed to compile calculator WASM: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Copy to fixtures
    let wasm_source =
        calculator_dir.join("target/wasm32-wasip1/release/calculator-wasm.wasm");
    let wasm_dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/wasm-tools/calculator/calculator.wasm");

    std::fs::create_dir_all(wasm_dest.parent().unwrap()).ok();
    std::fs::copy(wasm_source, &wasm_dest)
        .expect("Failed to copy WASM module to fixtures");

    wasm_dest
}

/// Helper to create an executor with loaded WASM tools
async fn create_test_executor() -> WasmToolExecutor {
    ensure_calculator_wasm().await;

    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm-tools");

    let provider = Arc::new(InMemoryCredentialProvider::new());
    let _runtime =
        Arc::new(rmcp::wasm::WasmRuntime::new().expect("Failed to create runtime"));
    let registry = Arc::new(
        WasmToolRegistry::load_from_directory(&fixtures_dir, provider)
            .expect("Failed to load WASM tools"),
    );

    WasmToolExecutor::new(registry)
}

/// Helper to extract text content from a CallToolResult
fn get_result_text(result: &CallToolResult) -> String {
    result.content[0]
        .as_text()
        .expect("Should be text content")
        .text
        .clone()
}

/// Helper to parse JSON from result text
fn parse_result_json(result: &CallToolResult) -> serde_json::Value {
    let text = get_result_text(result);
    serde_json::from_str(&text).expect("Should parse as JSON")
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_calculator_wasm_basic_operations() {
    let executor = create_test_executor().await;

    // Test addition
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "add",
                "a": 10,
                "b": 5
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Addition should succeed");

    assert_eq!(result.is_error, Some(false));
    let output = parse_result_json(&result);
    assert_eq!(output["result"], 15.0);

    // Test subtraction
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "subtract",
                "a": 10,
                "b": 3
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Subtraction should succeed");

    assert_eq!(result.is_error, Some(false));
    let output = parse_result_json(&result);
    assert_eq!(output["result"], 7.0);

    // Test multiplication
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "multiply",
                "a": 4,
                "b": 7
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Multiplication should succeed");

    assert_eq!(result.is_error, Some(false));
    let output = parse_result_json(&result);
    assert_eq!(output["result"], 28.0);

    // Test division
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "divide",
                "a": 20,
                "b": 4
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Division should succeed");

    assert_eq!(result.is_error, Some(false));
    let output = parse_result_json(&result);
    assert_eq!(output["result"], 5.0);
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_calculator_wasm_error_handling() {
    let executor = create_test_executor().await;

    // Test division by zero
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "divide",
                "a": 10,
                "b": 0
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Should handle division by zero gracefully");

    assert_eq!(result.is_error, Some(true));
    let text = get_result_text(&result);
    assert!(text.contains("Division by zero"));

    // Test invalid operation
    let result = executor
        .execute(
            "calculator",
            json!({
                "operation": "modulo",
                "a": 10,
                "b": 3
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Should handle invalid operation");

    assert_eq!(result.is_error, Some(true));
    let text = get_result_text(&result);
    assert!(text.contains("Unknown operation"));
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_wasm_tool_idempotency() {
    // Test that calling the same tool with the same inputs produces the same results
    let executor = create_test_executor().await;

    let arguments = json!({
        "operation": "multiply",
        "a": 42,
        "b": std::f64::consts::PI
    })
    .as_object()
    .unwrap()
    .clone();

    // Call the same operation multiple times
    let mut results = Vec::new();
    for _ in 0..5 {
        let result = executor
            .execute("calculator", arguments.clone())
            .await
            .expect("Calculator should work");
        results.push(get_result_text(&result));
    }

    // All results should be identical
    let first_result = &results[0];
    for result in &results[1..] {
        assert_eq!(result, first_result, "Results should be idempotent");
    }
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_wasm_tool_statelessness() {
    // Test that tools don't maintain state between calls
    let executor = create_test_executor().await;

    // Perform a series of calculations
    // Each should be independent - no state carried over
    let operations = vec![
        ("add", 100.0, 50.0, 150.0),
        ("subtract", 10.0, 3.0, 7.0), // Not 147.0 (150 - 3)
        ("multiply", 5.0, 6.0, 30.0), // Not 42.0 (7 * 6)
        ("divide", 100.0, 4.0, 25.0), // Not 7.5 (30 / 4)
    ];

    for (op, a, b, expected) in operations {
        let result = executor
            .execute(
                "calculator",
                json!({
                    "operation": op,
                    "a": a,
                    "b": b
                })
                .as_object()
                .unwrap()
                .clone(),
            )
            .await
            .expect("Operation should succeed");

        let output = parse_result_json(&result);
        let actual_result = output["result"].as_f64().expect("Should have result");
        assert!(
            (actual_result - expected).abs() < 0.001,
            "Operation {} should produce {}, got {}",
            op,
            expected,
            actual_result
        );
    }
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_wasm_tool_chaining() {
    // Test that outputs from one tool can be used as inputs to another
    let executor = create_test_executor().await;

    // Chain of calculations: ((10 + 5) * 2) - 3 = 27

    // Step 1: 10 + 5 = 15
    let result1 = executor
        .execute(
            "calculator",
            json!({
                "operation": "add",
                "a": 10,
                "b": 5
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Step 1 should succeed");

    let output1 = parse_result_json(&result1);
    let intermediate1 = output1["result"].as_f64().expect("Should have result");

    // Step 2: 15 * 2 = 30
    let result2 = executor
        .execute(
            "calculator",
            json!({
                "operation": "multiply",
                "a": intermediate1,
                "b": 2
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Step 2 should succeed");

    let output2 = parse_result_json(&result2);
    let intermediate2 = output2["result"].as_f64().expect("Should have result");

    // Step 3: 30 - 3 = 27
    let result3 = executor
        .execute(
            "calculator",
            json!({
                "operation": "subtract",
                "a": intermediate2,
                "b": 3
            })
            .as_object()
            .unwrap()
            .clone(),
        )
        .await
        .expect("Step 3 should succeed");

    let output3 = parse_result_json(&result3);
    let final_result = output3["result"].as_f64().expect("Should have result");

    assert!(
        (final_result - 27.0).abs() < 0.001,
        "Chain calculation should produce 27, got {}",
        final_result
    );
}

#[tokio::test]
#[ignore] // Ignore by default as it requires wasm32-wasip1 target
async fn test_multiple_wasm_tools_concurrent() {
    // Test that multiple WASM tools can run concurrently without interference
    ensure_calculator_wasm().await;

    let fixtures_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm-tools");

    let provider = Arc::new(InMemoryCredentialProvider::new());
    let _runtime =
        Arc::new(rmcp::wasm::WasmRuntime::new().expect("Failed to create runtime"));
    let registry = Arc::new(
        WasmToolRegistry::load_from_directory(&fixtures_dir, provider)
            .expect("Failed to load WASM tools"),
    );
    let executor = Arc::new(WasmToolExecutor::new(registry));

    // Launch multiple concurrent calculations
    let mut handles = Vec::new();

    for i in 0..10 {
        let executor_clone = executor.clone();
        let handle = tokio::spawn(async move {
            let result = executor_clone
                .execute(
                    "calculator",
                    json!({
                        "operation": "multiply",
                        "a": i as f64,
                        "b": i as f64
                    })
                    .as_object()
                    .unwrap()
                    .clone(),
                )
                .await
                .expect("Calculation should succeed");

            let output = parse_result_json(&result);
            let value = output["result"].as_f64().expect("Should have result");

            (i, value)
        });

        handles.push(handle);
    }

    // Collect results and verify
    for handle in handles {
        let (i, result) = handle.await.expect("Task should complete");
        let expected = (i * i) as f64;
        assert!(
            (result - expected).abs() < 0.001,
            "Concurrent calculation {} should produce {}, got {}",
            i,
            expected,
            result
        );
    }
}
