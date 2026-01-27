//! Integration tests for WASM metering functionality

#![cfg(feature = "wasm-tools")]

use std::time::Duration;

use mcpkit_rs::wasm::{
    ComputeUnits, DisplayFormat, EnforcementMode, FuelMetrics, MemoryLimits, MeteringConfig,
    MeteringMonitor, WasmContext, WasmRuntime,
};
use tokio::time::timeout;

/// Helper to compile WAT to WASM
fn compile_wat(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("Failed to compile WAT")
}

/// Load a WAT fixture file
fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/{}", name);
    let wat = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("Failed to read {}", path));
    compile_wat(&wat)
}

#[tokio::test]
async fn test_metering_config_enforcement_denied() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let infinite_loop = load_fixture("infinite_loop.wat");

    let context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(10_000),
        enforcement: EnforcementMode::Strict,
        ..Default::default()
    });

    let result = runtime
        .execute_bytes_with_metering(&infinite_loop, context)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("fuel limit"),
        "Expected fuel limit error, got: {}",
        err
    );
}

#[tokio::test]
async fn test_metering_config_enforcement_allowed() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let simple_loop = load_fixture("simple_loop.wat");

    let context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(100_000),
        enforcement: EnforcementMode::Strict,
        ..Default::default()
    });

    let result = runtime
        .execute_bytes_with_metering(&simple_loop, context)
        .await;

    assert!(result.is_ok());
    let (_output, metrics) = result.unwrap();
    assert!(metrics.is_some());

    let metrics = metrics.unwrap();
    assert!(metrics.compute_units.0 > 0);
    assert!(metrics.compute_units.0 < 100_000);
}

#[tokio::test]
async fn test_metering_monotonic_increase() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");

    let wat = r#"
        (module
          (func $count (export "_start")
            (local $i i32)
            (local.set $i (i32.const 0))
            (loop $continue
              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (br_if $continue (i32.lt_u (local.get $i) (i32.const 10000)))
            )
          )
          (memory (export "memory") 1)
        )
    "#;
    let wasm = compile_wat(wat);

    let monitor = MeteringMonitor::new(100);
    let receiver = monitor.receiver.clone();

    let context = WasmContext::new()
        .with_metering(MeteringConfig {
            enabled: true,
            max_compute_units: Some(1_000_000),
            enable_monitoring: true,
            ..Default::default()
        })
        .with_monitor(monitor);

    let exec_handle =
        tokio::spawn(async move { runtime.execute_bytes_with_metering(&wasm, context).await });

    let mut updates = Vec::new();
    let mut rx = receiver.lock().await;

    while let Ok(Some(update)) = timeout(Duration::from_millis(10), rx.recv()).await {
        updates.push(update.consumed.0);
    }

    let result = exec_handle.await.unwrap();
    assert!(result.is_ok());

    for window in updates.windows(2) {
        assert!(
            window[1] >= window[0],
            "Fuel consumption should be monotonically increasing: {} -> {}",
            window[0],
            window[1]
        );
    }
}

#[tokio::test]
async fn test_metering_live_monitoring() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let simple_loop = load_fixture("simple_loop.wat");

    let monitor = MeteringMonitor::new(10);
    let receiver = monitor.receiver.clone();

    let context = WasmContext::new()
        .with_metering(MeteringConfig {
            enabled: true,
            max_compute_units: Some(100_000),
            enable_monitoring: true,
            ..Default::default()
        })
        .with_monitor(monitor);

    let exec_handle = tokio::spawn(async move {
        runtime
            .execute_bytes_with_metering(&simple_loop, context)
            .await
    });

    let mut live_updates = Vec::new();
    let mut rx = receiver.lock().await;

    while let Ok(Some(update)) = timeout(Duration::from_millis(100), rx.recv()).await {
        live_updates.push(update);
    }

    let result = exec_handle.await.unwrap();
    assert!(result.is_ok());

    assert!(
        !live_updates.is_empty(),
        "Should receive live monitoring updates"
    );

    let last_update = live_updates.last().unwrap();
    assert!(last_update.consumed.0 > 0, "Should have consumed fuel");
}

#[tokio::test]
async fn test_metering_post_execution_metrics() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let noop = load_fixture("noop.wat");

    let context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(10_000),
        ..Default::default()
    });

    let result = runtime.execute_bytes_with_metering(&noop, context).await;

    assert!(result.is_ok());
    let (_output, metrics) = result.unwrap();
    assert!(metrics.is_some());

    let metrics = metrics.unwrap();

    assert!(metrics.compute_units.0 > 0);
    assert!(metrics.execution_time.as_nanos() > 0);

    let exact = metrics.compute_units.exact();
    assert!(exact.contains(" CU"));
    assert!(exact.contains(char::is_numeric));

    let abbreviated = metrics.compute_units.abbreviated();
    assert!(abbreviated.ends_with(" CU"));

    let summary = metrics.summary();
    assert!(summary.contains("⚡"));
}

#[tokio::test]
async fn test_metering_default_enabled() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let simple_loop = load_fixture("simple_loop.wat");

    // Default context should have metering enabled with tracking mode
    let context = WasmContext::new();

    let result = runtime
        .execute_bytes_with_metering(&simple_loop, context)
        .await;

    assert!(result.is_ok());
    let (_output, metrics) = result.unwrap();

    assert!(metrics.is_some(), "Metering should be enabled by default");

    let metrics = metrics.unwrap();
    assert!(
        metrics.compute_units.0 > 0,
        "Should track fuel consumption by default"
    );
}

#[tokio::test]
async fn test_metering_can_be_disabled() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let simple_loop = load_fixture("simple_loop.wat");

    // Explicitly disable metering
    let context = WasmContext::new().without_metering();

    let result = runtime
        .execute_bytes_with_metering(&simple_loop, context)
        .await;

    assert!(result.is_ok());
    let (_output, metrics) = result.unwrap();

    assert!(
        metrics.is_none(),
        "Metering should be disabled when explicitly turned off"
    );
}

#[tokio::test]
async fn test_no_metering_config_allows_perpetual_execution() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");

    let wat = r#"
        (module
          (func $big_loop (export "_start")
            (local $i i32)
            (local.set $i (i32.const 0))
            (loop $continue
              (local.set $i (i32.add (local.get $i) (i32.const 1)))
              (br_if $continue (i32.lt_u (local.get $i) (i32.const 100000)))
            )
          )
          (memory (export "memory") 1)
        )
    "#;
    let wasm = compile_wat(wat);

    let context = WasmContext::new();

    let result = timeout(
        Duration::from_secs(5),
        runtime.execute_bytes_with_metering(&wasm, context),
    )
    .await;

    assert!(
        result.is_ok(),
        "Should complete within timeout without config limits"
    );
    let result = result.unwrap();
    assert!(result.is_ok(), "Should execute successfully");
}

#[tokio::test]
async fn test_metering_with_output() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let hello = load_fixture("hello.wat");

    let context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(10_000),
        ..Default::default()
    });

    let result = runtime.execute_bytes_with_metering(&hello, context).await;

    assert!(result.is_ok());
    let (output, metrics) = result.unwrap();

    assert_eq!(output, b"Hello, World!\n");
    assert!(metrics.is_some());

    let metrics = metrics.unwrap();
    assert!(metrics.compute_units.0 > 0);
}

#[tokio::test]
async fn test_metering_display_formats() {
    let metrics = FuelMetrics {
        compute_units: ComputeUnits::new(1_234_567),
        execution_time: Duration::from_millis(100),
        units_per_second: 12_345_670,
        peak_rate: Some(15_000_000),
        instruction_count: Some(1_000_000),
    };

    let minimal = metrics.display(DisplayFormat::Minimal);
    assert_eq!(minimal, "⚡ 1.2M CU");

    let detailed = metrics.display(DisplayFormat::Detailed);
    assert!(detailed.contains("1,234,567 CU"));
    assert!(detailed.contains("1.235e6 CU"));
    assert!(detailed.contains("12.3M CU/s"));

    let json = metrics.display(DisplayFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["compute_units"], 1_234_567);
}

#[tokio::test]
async fn test_memory_limits_with_metering() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");

    let wat = r#"
        (module
          (func $allocate (export "_start")
            ;; Try to grow memory by 10 pages (640KB total with initial 1 page)
            ;; If it fails (returns -1), trap
            (if (i32.eq (memory.grow (i32.const 10)) (i32.const -1))
              (then unreachable)
            )
          )
          (memory (export "memory") 1)
        )
    "#;
    let wasm = compile_wat(wat);

    let context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(100_000),
        memory_limits: MemoryLimits {
            max_memory: 1024 * 64, // 64KB (1 page)
            soft_limit: None,
            max_tables: 10,
            max_instances: 1,
        },
        ..Default::default()
    });

    let result = runtime.execute_bytes_with_metering(&wasm, context).await;

    assert!(result.is_err(), "Should fail on memory limit");
}

#[tokio::test]
async fn test_enforcement_modes() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let simple_loop = load_fixture("simple_loop.wat");

    let tracking_context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(1),
        enforcement: EnforcementMode::Tracking,
        ..Default::default()
    });

    let result = runtime
        .execute_bytes_with_metering(&simple_loop, tracking_context)
        .await;
    assert!(result.is_ok(), "Tracking mode should not enforce limits");

    let strict_context = WasmContext::new().with_metering(MeteringConfig {
        enabled: true,
        max_compute_units: Some(1),
        enforcement: EnforcementMode::Strict,
        ..Default::default()
    });

    let result = runtime
        .execute_bytes_with_metering(&simple_loop, strict_context)
        .await;
    assert!(result.is_err(), "Strict mode should enforce limits");
}
