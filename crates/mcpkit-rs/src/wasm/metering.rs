//! WASM metering system for tracking compute usage

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Universal compute unit measurement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeUnits(pub u64);

impl ComputeUnits {
    /// Create from raw units
    pub const fn new(units: u64) -> Self {
        Self(units)
    }

    /// Format for exact display with comma separators
    pub fn exact(&self) -> String {
        format!("{:} CU", format_with_commas(self.0))
    }

    /// Format for abbreviated display
    pub fn abbreviated(&self) -> String {
        match self.0 {
            n if n < 1_000 => format!("{} CU", n),
            n if n < 1_000_000 => format!("{:.1}K CU", n as f64 / 1_000.0),
            n if n < 1_000_000_000 => format!("{:.1}M CU", n as f64 / 1_000_000.0),
            _ => format!("{:.1}B CU", self.0 as f64 / 1_000_000_000.0),
        }
    }

    /// Format for scientific notation
    pub fn scientific(&self) -> String {
        format!("{:.3e} CU", self.0 as f64)
    }
}

/// Helper to format numbers with comma separators
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();

    for (count, ch) in s.chars().rev().enumerate() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }

    result.chars().rev().collect()
}

/// Unified metering result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelMetrics {
    /// Total compute units consumed
    pub compute_units: ComputeUnits,

    /// Total execution time
    pub execution_time: Duration,

    /// Average units per second
    pub units_per_second: u64,

    /// Peak consumption rate (units/sec)
    pub peak_rate: Option<u64>,

    /// Total instructions executed (if available)
    pub instruction_count: Option<u64>,
}

impl FuelMetrics {
    /// Human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "⚡ {} in {:.2}s ({}/s avg)",
            self.compute_units.abbreviated(),
            self.execution_time.as_secs_f64(),
            ComputeUnits::new(self.units_per_second).abbreviated(),
        )
    }

    /// Detailed report
    pub fn detailed_report(&self) -> String {
        let mut report = String::new();

        report.push_str(&format!(
            "Compute Usage Report\n\
             ====================\n\
             Total:       {}\n\
             Abbreviated: {}\n\
             Scientific:  {}\n\
             Time:        {:.3}s\n\
             Rate (avg):  {}/s\n",
            self.compute_units.exact(),
            self.compute_units.abbreviated(),
            self.compute_units.scientific(),
            self.execution_time.as_secs_f64(),
            ComputeUnits::new(self.units_per_second).abbreviated(),
        ));

        if let Some(peak) = self.peak_rate {
            report.push_str(&format!(
                "Rate (peak): {}/s\n",
                ComputeUnits::new(peak).abbreviated()
            ));
        }

        if let Some(count) = self.instruction_count {
            report.push_str(&format!("Instructions: {}\n", format_with_commas(count)));
        }

        report
    }

    /// Display with specified format
    pub fn display(&self, format: DisplayFormat) -> String {
        match format {
            DisplayFormat::Minimal => format!("⚡ {}", self.compute_units.abbreviated()),
            DisplayFormat::Detailed => self.detailed_report(),
            DisplayFormat::Json => serde_json::to_string_pretty(&self).unwrap(),
            DisplayFormat::ProgressBar => {
                // For now, just show abbreviated format
                // Could be enhanced with actual progress bar if we have limits
                self.summary()
            }
        }
    }
}

/// Display format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayFormat {
    /// Minimal single-line display
    Minimal,
    /// Detailed multi-line display
    Detailed,
    /// JSON for programmatic consumption
    Json,
    /// Progress bar visualization
    ProgressBar,
}

impl Default for DisplayFormat {
    fn default() -> Self {
        Self::Minimal
    }
}

/// Sampling strategy for real-time monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingStrategy {
    /// Minimum units between samples
    #[serde(default = "default_unit_threshold")]
    pub unit_threshold: u64,

    /// Minimum time between samples (ms)
    #[serde(default = "default_time_threshold_ms")]
    pub time_threshold_ms: u64,

    /// Adaptive rate based on execution speed
    #[serde(default = "default_adaptive")]
    pub adaptive: bool,
}

fn default_unit_threshold() -> u64 {
    100_000
}

fn default_time_threshold_ms() -> u64 {
    100
}

fn default_adaptive() -> bool {
    true
}

impl Default for SamplingStrategy {
    fn default() -> Self {
        Self {
            unit_threshold: default_unit_threshold(),
            time_threshold_ms: default_time_threshold_ms(),
            adaptive: default_adaptive(),
        }
    }
}

impl SamplingStrategy {
    /// Determine if we should sample now
    pub fn should_sample(
        &self,
        units_since_last: u64,
        time_since_last: Duration,
        current_rate: u64,
    ) -> bool {
        // Always respect time threshold
        if time_since_last.as_millis() < self.time_threshold_ms as u128 {
            return false;
        }

        // Check unit threshold
        if units_since_last < self.unit_threshold {
            return false;
        }

        // Adaptive sampling for high-speed execution
        if self.adaptive && current_rate > 1_000_000_000 {
            // For very fast execution, increase threshold
            return units_since_last > self.unit_threshold * 10;
        }

        true
    }
}

/// Memory limits with defense-in-depth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLimits {
    /// Maximum memory in bytes
    pub max_memory: usize,

    /// Soft limit for warnings
    pub soft_limit: Option<usize>,

    /// Maximum table elements
    #[serde(default = "default_max_tables")]
    pub max_tables: u32,

    /// Maximum module instances
    #[serde(default = "default_max_instances")]
    pub max_instances: u32,
}

fn default_max_tables() -> u32 {
    10
}

fn default_max_instances() -> u32 {
    100
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_memory: 512 * 1024 * 1024, // 512MB default
            soft_limit: None,
            max_tables: default_max_tables(),
            max_instances: default_max_instances(),
        }
    }
}

impl MemoryLimits {
    /// Parse human-readable size formats like "512Mi", "1Gi", "100Ki"
    pub fn parse_size(input: &str) -> Result<usize, String> {
        let digit_end = input
            .find(|c: char| c.is_alphabetic())
            .unwrap_or(input.len());

        let (number_str, unit) = input.split_at(digit_end);
        let base: usize = number_str
            .parse()
            .map_err(|e| format!("Invalid number: {}", e))?;

        let multiplier = match unit {
            "Ki" => 1024,
            "Mi" => 1024 * 1024,
            "Gi" => 1024 * 1024 * 1024,
            "K" => 1000,
            "M" => 1000 * 1000,
            "G" => 1000 * 1000 * 1000,
            "" => 1,
            _ => return Err(format!("Unknown unit: {}", unit)),
        };

        Ok(base * multiplier)
    }
}

/// Enforcement mode for metering
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementMode {
    /// Only track, don't enforce limits
    Tracking,
    /// Warn when approaching limits
    Warning {
        /// Threshold as percentage (0.0 to 1.0)
        threshold: f32,
    },
    /// Hard stop at limit
    Strict,
}

impl Eq for EnforcementMode {}

impl Default for EnforcementMode {
    fn default() -> Self {
        Self::Strict
    }
}

/// Metering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteringConfig {
    /// Enable metering
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Maximum compute units
    pub max_compute_units: Option<u64>,

    /// Memory limits (defense-in-depth)
    #[serde(default)]
    pub memory_limits: MemoryLimits,

    /// Enable real-time monitoring
    #[serde(default)]
    pub enable_monitoring: bool,

    /// Sampling strategy
    #[serde(default)]
    pub sampling: SamplingStrategy,

    /// Display format
    #[serde(default)]
    pub display_format: DisplayFormat,

    /// Enforcement mode
    #[serde(default)]
    pub enforcement: EnforcementMode,
}

fn default_enabled() -> bool {
    false
}

impl Default for MeteringConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_compute_units: None,
            memory_limits: MemoryLimits::default(),
            enable_monitoring: false,
            sampling: SamplingStrategy::default(),
            display_format: DisplayFormat::default(),
            enforcement: EnforcementMode::default(),
        }
    }
}

/// Real-time monitoring channel
pub struct MeteringMonitor {
    sender: mpsc::Sender<FuelUpdate>,
    pub receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<FuelUpdate>>>,
    closed: Arc<AtomicBool>,
}

impl MeteringMonitor {
    /// Create a new monitor with bounded channel
    pub fn new(buffer_size: usize) -> Self {
        let (sender, receiver) = mpsc::channel(buffer_size);

        Self {
            sender,
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Send an update (non-blocking)
    pub fn send_update(&self, update: FuelUpdate) {
        // Use try_send to avoid blocking
        let _ = self.sender.try_send(update);
    }

    /// Send final update and close
    pub fn send_final(&self, update: FuelUpdate) {
        let _ = self.sender.try_send(update);
        self.closed.store(true, Ordering::Relaxed);
    }

    /// Check if closed
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Start display task for CLI
    pub fn start_display_task(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut last_display = Instant::now();
            let mut receiver = self.receiver.lock().await;

            while let Some(update) = receiver.recv().await {
                // Throttle display updates to 10 FPS
                if last_display.elapsed() > Duration::from_millis(100) {
                    // Clear line and print update
                    print!(
                        "\r⚡ {} @ {}/s",
                        update.consumed.abbreviated(),
                        ComputeUnits::new(update.rate).abbreviated()
                    );
                    use std::io::{self, Write};
                    io::stdout().flush().unwrap();

                    last_display = Instant::now();
                }

                if self.is_closed() {
                    break;
                }
            }

            // Final newline
            println!();
        })
    }
}

/// Fuel consumption update
#[derive(Debug, Clone)]
pub struct FuelUpdate {
    /// Units consumed so far
    pub consumed: ComputeUnits,
    /// Units remaining (if known)
    pub remaining: Option<ComputeUnits>,
    /// Current rate (units per second)
    pub rate: u64,
    /// Update timestamp
    pub timestamp: Instant,
}

/// Runtime-agnostic metering trait
pub trait RuntimeMetering {
    /// Get native units consumed
    fn native_units(&self) -> u64;

    /// Convert to normalized compute units
    fn to_compute_units(&self) -> ComputeUnits {
        // 1:1 mapping for both runtimes currently
        ComputeUnits::new(self.native_units())
    }

    /// Runtime-specific unit name
    fn unit_name(&self) -> &'static str;
}

/// Wasmtime fuel wrapper
pub struct WasmtimeFuel(pub u64);

impl RuntimeMetering for WasmtimeFuel {
    fn native_units(&self) -> u64 {
        self.0
    }

    fn unit_name(&self) -> &'static str {
        "fuel"
    }
}

/// WasmEdge gas wrapper
pub struct WasmEdgeGas(pub u64);

impl RuntimeMetering for WasmEdgeGas {
    fn native_units(&self) -> u64 {
        self.0
    }

    fn unit_name(&self) -> &'static str {
        "gas"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_formatting() {
        let cu = ComputeUnits::new(1_234_567);
        assert_eq!(cu.exact(), "1,234,567 CU");
    }

    #[test]
    fn test_abbreviated_formatting() {
        assert_eq!(ComputeUnits::new(999).abbreviated(), "999 CU");
        assert_eq!(ComputeUnits::new(1_500).abbreviated(), "1.5K CU");
        assert_eq!(ComputeUnits::new(1_500_000).abbreviated(), "1.5M CU");
        assert_eq!(ComputeUnits::new(2_500_000_000).abbreviated(), "2.5B CU");
    }

    #[test]
    fn test_scientific_formatting() {
        let cu = ComputeUnits::new(1_234_567);
        assert_eq!(cu.scientific(), "1.235e6 CU");
    }

    #[test]
    fn test_human_readable_parsing() {
        assert_eq!(MemoryLimits::parse_size("512Ki").unwrap(), 524_288);
        assert_eq!(MemoryLimits::parse_size("256Mi").unwrap(), 268_435_456);
        assert_eq!(MemoryLimits::parse_size("2Gi").unwrap(), 2_147_483_648);
        assert_eq!(MemoryLimits::parse_size("100").unwrap(), 100);
    }

    #[test]
    fn test_invalid_format() {
        assert!(MemoryLimits::parse_size("ABC").is_err());
        assert!(MemoryLimits::parse_size("512Xi").is_err());
    }

    #[test]
    fn test_sampling_strategy() {
        let strategy = SamplingStrategy::default();

        // Should not sample before time threshold
        assert!(!strategy.should_sample(
            200_000,                   // Above unit threshold
            Duration::from_millis(50), // Below time threshold
            1_000_000
        ));

        // Should not sample before unit threshold
        assert!(!strategy.should_sample(
            50_000,                     // Below unit threshold
            Duration::from_millis(200), // Above time threshold
            1_000_000
        ));

        // Should sample when both thresholds met
        assert!(strategy.should_sample(
            200_000,                    // Above unit threshold
            Duration::from_millis(200), // Above time threshold
            1_000_000
        ));
    }
}
