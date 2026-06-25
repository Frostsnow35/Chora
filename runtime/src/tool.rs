//! Tool execution types and traits.
//!
//! This module defines how tools are represented, executed, and how their results
//! flow back to agents. The design follows the project principles:
//!
//! - **P4 (End-to-End)**: the runtime does not interpret tool results — it passes
//!   them through to the agent, which decides what to do with them.
//! - **P1 (Mechanism/Policy)**: `ToolExecutor` is a mechanism; which tools an agent
//!   is allowed to call is a policy decision enforced elsewhere.
//!
//! See `../../rfc/001-agent-process.md` §8 for the validation plan that motivated
//! this module (Agent 1 — Tool-Using / Calculator).

use crate::ToolCallRequest;
use serde::{Deserialize, Serialize};

/// The outcome of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolOutcome {
    /// Tool completed successfully, returning a JSON value.
    Success(serde_json::Value),
    /// Tool failed with an error code and message.
    Error {
        /// Machine-readable error code (e.g. "INVALID_ARGS", "TIMEOUT").
        code: String,
        /// Human-readable error description.
        message: String,
    },
}

/// The result of executing a single tool call.
///
/// Carries the `call_id` from the original request so the agent can match
/// results to requests when multiple calls are in flight.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    /// Matches the `call_id` in the originating `ToolCallRequest`.
    pub call_id: String,
    /// Which tool produced this result.
    pub tool_name: String,
    /// Success value or error.
    pub outcome: ToolOutcome,
}

/// Mechanism for executing tool calls.
///
/// Implementations may wrap local functions, RPC endpoints, or sandboxed
/// subprocesses. The runtime treats all executors identically — it does not
/// inspect the outcome (P4).
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call and return the result.
    ///
    /// Implementations must not panic. Invalid tool names produce a
    /// `ToolOutcome::Error` result rather than a Rust-level error, so the
    /// runtime can always deliver a well-formed `ToolResult` back to the agent.
    fn execute(&self, req: &ToolCallRequest) -> ToolResult;

    /// Check whether this executor has a tool registered under `name`.
    fn has_tool(&self, name: &str) -> bool;

    /// List all tool names registered with this executor.
    fn tool_names(&self) -> Vec<&str>;
}

// ---------------------------------------------------------------------------
// Built-in tools
// ---------------------------------------------------------------------------

/// A tool that returns its arguments verbatim.
///
/// Useful for testing the full tool-call round-trip without any transformation.
#[derive(Debug, Clone, Copy)]
pub struct EchoTool;

impl EchoTool {
    /// Tool name exposed to agents.
    pub const NAME: &'static str = "echo";

    /// Execute the echo tool: return `args` as the success value.
    pub fn execute(req: &ToolCallRequest) -> ToolResult {
        ToolResult {
            call_id: req.call_id.clone(),
            tool_name: Self::NAME.to_string(),
            outcome: ToolOutcome::Success(req.args.clone()),
        }
    }
}

/// A tool that adds two numbers.
///
/// Expects args in the form `{"a": N, "b": N}` and returns `{"result": N}`.
/// Returns an error outcome for malformed input.
#[derive(Debug, Clone, Copy)]
pub struct AddTool;

impl AddTool {
    /// Tool name exposed to agents.
    pub const NAME: &'static str = "add";

    /// Execute the add tool.
    pub fn execute(req: &ToolCallRequest) -> ToolResult {
        let result = (|| {
            let a = req
                .args
                .get("a")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| "missing or non-numeric field 'a'".to_string())?;
            let b = req
                .args
                .get("b")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| "missing or non-numeric field 'b'".to_string())?;
            Ok::<f64, String>(a + b)
        })();

        let outcome = match result {
            Ok(sum) => {
                ToolOutcome::Success(serde_json::json!({ "result": sum }))
            }
            Err(message) => ToolOutcome::Error {
                code: "INVALID_ARGS".to_string(),
                message,
            },
        };

        ToolResult {
            call_id: req.call_id.clone(),
            tool_name: Self::NAME.to_string(),
            outcome,
        }
    }
}

/// A registry of built-in tools that dispatches by name.
///
/// Unknown tool names produce a `ToolOutcome::Error` with code
/// `"UNKNOWN_TOOL"` rather than panicking, keeping the runtime's control
/// flow uniform.
#[derive(Debug, Clone, Default)]
pub struct BuiltinToolExecutor {
    /// Extra tool names registered by the user (on top of echo + add).
    extra_names: Vec<String>,
}

impl BuiltinToolExecutor {
    /// Create an executor with the default tools (`echo`, `add`).
    pub fn new() -> Self {
        Self {
            extra_names: vec![],
        }
    }

    /// Register an additional tool name.
    ///
    /// The actual execution still falls through to the built-in dispatch, so
    /// this is mainly useful for tests that want to verify `has_tool` /
    /// `tool_names` behaviour.
    pub fn register(mut self, name: impl Into<String>) -> Self {
        self.extra_names.push(name.into());
        self
    }
}

impl ToolExecutor for BuiltinToolExecutor {
    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        match req.tool_name.as_str() {
            EchoTool::NAME => EchoTool::execute(req),
            AddTool::NAME => AddTool::execute(req),
            unknown => ToolResult {
                call_id: req.call_id.clone(),
                tool_name: unknown.to_string(),
                outcome: ToolOutcome::Error {
                    code: "UNKNOWN_TOOL".to_string(),
                    message: format!("no tool registered with name '{}'", unknown),
                },
            },
        }
    }

    fn has_tool(&self, name: &str) -> bool {
        matches!(name, EchoTool::NAME | AddTool::NAME)
            || self.extra_names.iter().any(|n| n == name)
    }

    fn tool_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = vec![EchoTool::NAME, AddTool::NAME];
        for n in &self.extra_names {
            names.push(n.as_str());
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolCallRequest;

    fn make_request(tool_name: &str, args: serde_json::Value) -> ToolCallRequest {
        ToolCallRequest {
            call_id: "test-call-1".to_string(),
            tool_name: tool_name.to_string(),
            args,
        }
    }

    // --- EchoTool ----------------------------------------------------------

    #[test]
    fn test_echo_tool_returns_args() {
        let req = make_request("echo", serde_json::json!({"hello": "world"}));
        let result = EchoTool::execute(&req);
        assert_eq!(result.call_id, "test-call-1");
        assert_eq!(result.tool_name, "echo");
        assert_eq!(
            result.outcome,
            ToolOutcome::Success(serde_json::json!({"hello": "world"}))
        );
    }

    #[test]
    fn test_echo_tool_handles_null() {
        let req = make_request("echo", serde_json::Value::Null);
        let result = EchoTool::execute(&req);
        assert_eq!(result.outcome, ToolOutcome::Success(serde_json::Value::Null));
    }

    // --- AddTool -----------------------------------------------------------

    #[test]
    fn test_add_tool_computes_sum() {
        let req = make_request("add", serde_json::json!({"a": 3, "b": 4}));
        let result = AddTool::execute(&req);
        assert_eq!(result.tool_name, "add");
        assert_eq!(
            result.outcome,
            ToolOutcome::Success(serde_json::json!({"result": 7.0}))
        );
    }

    #[test]
    fn test_add_tool_handles_floats() {
        let req = make_request("add", serde_json::json!({"a": 1.5, "b": 2.5}));
        let result = AddTool::execute(&req);
        assert_eq!(
            result.outcome,
            ToolOutcome::Success(serde_json::json!({"result": 4.0}))
        );
    }

    #[test]
    fn test_add_tool_handles_invalid_args() {
        let req = make_request("add", serde_json::json!({"a": "not_a_number"}));
        let result = AddTool::execute(&req);
        match result.outcome {
            ToolOutcome::Error { code, message } => {
                assert_eq!(code, "INVALID_ARGS");
                assert!(message.contains("'a'") || message.contains("'b'"));
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_add_tool_missing_field() {
        let req = make_request("add", serde_json::json!({}));
        let result = AddTool::execute(&req);
        assert!(matches!(result.outcome, ToolOutcome::Error { .. }));
    }

    // --- BuiltinToolExecutor -----------------------------------------------

    #[test]
    fn test_builtin_executor_dispatches_echo() {
        let exec = BuiltinToolExecutor::new();
        let req = make_request("echo", serde_json::json!("ping"));
        let result = exec.execute(&req);
        assert_eq!(
            result.outcome,
            ToolOutcome::Success(serde_json::json!("ping"))
        );
    }

    #[test]
    fn test_builtin_executor_dispatches_add() {
        let exec = BuiltinToolExecutor::new();
        let req = make_request("add", serde_json::json!({"a": 10, "b": 20}));
        let result = exec.execute(&req);
        assert_eq!(
            result.outcome,
            ToolOutcome::Success(serde_json::json!({"result": 30.0}))
        );
    }

    #[test]
    fn test_builtin_executor_unknown_tool() {
        let exec = BuiltinToolExecutor::new();
        let req = make_request("nonexistent", serde_json::json!({}));
        let result = exec.execute(&req);
        match result.outcome {
            ToolOutcome::Error { code, message } => {
                assert_eq!(code, "UNKNOWN_TOOL");
                assert!(message.contains("nonexistent"));
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_builtin_executor_has_tool() {
        let exec = BuiltinToolExecutor::new();
        assert!(exec.has_tool("echo"));
        assert!(exec.has_tool("add"));
        assert!(!exec.has_tool("subtract"));
    }

    #[test]
    fn test_builtin_executor_lists_tools() {
        let exec = BuiltinToolExecutor::new();
        let names = exec.tool_names();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"add"));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_builtin_executor_register_extra() {
        let exec = BuiltinToolExecutor::new().register("custom");
        assert!(exec.has_tool("custom"));
        assert_eq!(exec.tool_names().len(), 3);
    }
}
