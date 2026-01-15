/// A simple calculator WASM module for testing MCP WASM tool integration
///
/// This module reads JSON input from stdin, performs calculations, and writes JSON output to stdout.
/// Expected input format:
/// {
///   "operation": "add" | "subtract" | "multiply" | "divide",
///   "a": number,
///   "b": number
/// }
///
/// Output format:
/// {
///   "result": number
/// }
///
/// Error format:
/// {
///   "error": "error message"
/// }

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

#[derive(Debug, Deserialize)]
struct CalculatorInput {
    operation: String,
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize)]
struct CalculatorOutput {
    result: f64,
}

#[derive(Debug, Serialize)]
struct ErrorOutput {
    error: String,
}

fn main() {
    // Read input from stdin
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        let error = ErrorOutput {
            error: format!("Failed to read input: {}", e),
        };
        let _ = serde_json::to_writer(io::stdout(), &error);
        return;
    }

    // Parse input JSON
    let request: CalculatorInput = match serde_json::from_str(&input) {
        Ok(req) => req,
        Err(e) => {
            let error = ErrorOutput {
                error: format!("Invalid input JSON: {}", e),
            };
            let _ = serde_json::to_writer(io::stdout(), &error);
            return;
        }
    };

    // Perform calculation
    let result = match request.operation.as_str() {
        "add" => request.a + request.b,
        "subtract" => request.a - request.b,
        "multiply" => request.a * request.b,
        "divide" => {
            if request.b == 0.0 {
                let error = ErrorOutput {
                    error: "Division by zero".to_string(),
                };
                let _ = serde_json::to_writer(io::stdout(), &error);
                return;
            }
            request.a / request.b
        }
        op => {
            let error = ErrorOutput {
                error: format!("Unknown operation: {}", op),
            };
            let _ = serde_json::to_writer(io::stdout(), &error);
            return;
        }
    };

    // Write output
    let output = CalculatorOutput { result };
    if let Err(e) = serde_json::to_writer(io::stdout(), &output) {
        let error = ErrorOutput {
            error: format!("Failed to write output: {}", e),
        };
        let _ = serde_json::to_writer(io::stderr(), &error);
    }
}