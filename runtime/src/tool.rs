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
//! See `../../rfc/001-agent-process.md` for the validation plan that motivated
//! this module (Agent 1 — Tool-Using / Calculator).

use crate::ToolCallRequest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
///
/// # Safety
///
/// This trait is safe. It does not require any unsafe operations or expose
/// unsafe methods.
///
/// # Design
///
/// The trait is intentionally minimal and mechanism-focused:
/// - **`execute()`** is the sole execution method
/// - **`has_tool()`** enables runtime tool discovery
/// - **`tool_names()`** enables introspection and listing
///
/// By keeping the interface small and focused, the runtime avoids becoming
/// a bottleneck (P1) while ensuring all tool executors are treated uniformly.
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call and return the result.
    ///
    /// Implementations must not panic. Invalid tool names produce a
    /// `ToolOutcome::Error` result rather than a Rust-level error, so the
    /// runtime can always deliver a well-formed `ToolResult` back to the agent.
    ///
    /// # Parameters
    ///
    /// - `req`: The tool call request containing:
    ///   - `call_id`: Stable identifier for matching results to requests
    ///   - `tool_name`: Name of the tool to invoke
    ///   - `args`: Arguments to pass to the tool, encoded as JSON
    ///
    /// # Returns
    ///
    /// A `ToolResult` containing:
    /// - `call_id`: Matches the request's `call_id`
    /// - `tool_name`: Which tool produced this result
    /// - `outcome`: Success value or error
    ///
    /// # Example
    ///
    /// ```rust
    /// let req = ToolCallRequest {
    ///     call_id: "call-123".to_string(),
    ///     tool_name: "add".to_string(),
    ///     args: json!({"a": 10, "b": 20}),
    /// };
    /// let result = executor.execute(&req);
    /// match result.outcome {
    ///     ToolOutcome::Success(value) => {
    ///         println!("Result: {}", value);
    ///     }
    ///     ToolOutcome::Error { code, message } => {
    ///         eprintln!("Error {}: {}", code, message);
    ///     }
    /// }
    /// ```
    fn execute(&self, req: &ToolCallRequest) -> ToolResult;

    /// Check whether this executor has a tool registered under `name`.
    fn has_tool(&self, name: &str) -> bool;

    /// List all tool names registered with this executor.
    fn tool_names(&self) -> Vec<&str>;
}

/// A registry of tools that dispatches by name.
///
/// The registry holds the actual tool implementations, allowing dynamic
/// registration without modifying the executor.
///
/// Unknown tool names produce a `ToolOutcome::Error` with code
/// "UNKNOWN_TOOL" rather than panicking, keeping the runtime's control
/// flow uniform.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolExecutor>>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a new tool with the given name.
    ///
    /// The provided tool implementation is stored in the registry and can be
    /// used for execution.
    pub fn register(&mut self, name: String, executor: Box<dyn ToolExecutor>) {
        self.tools.insert(name, executor);
    }

    /// Get a reference to the tool implementation by name.
    pub fn get(&self, name: &str) -> Option<&Box<dyn ToolExecutor>> {
        self.tools.get(name)
    }

    /// Execute a tool by name.
    ///
    /// Returns None if no tool with the given name is registered.
    pub fn execute(&self, name: &str, req: &ToolCallRequest) -> Option<ToolResult> {
        self.get(name).map(|t| t.execute(req))
    }

    /// Check whether this registry has a tool registered under `name`.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all tool names registered with this registry.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(AsRef::as_ref).collect()
    }
}

// ---------------------------------------------------------------------------
// Built-in tools
// ---------------------------------------------------------------------------

/// A tool that returns its arguments verbatim.
///
/// Useful for testing the full tool-call round-trip without any transformation.
#[derive(Debug, Default, Clone, Copy)]
struct EchoTool;

impl EchoTool {
    /// Tool name exposed to agents.
    const NAME: &'static str = "echo";

    /// Execute the echo tool: return `args` as the success value.
    fn execute(req: &ToolCallRequest) -> ToolResult {
        ToolResult {
            call_id: req.call_id.clone(),
            tool_name: Self::NAME.to_string(),
            outcome: ToolOutcome::Success(req.args.clone()),
        }
    }
}

impl ToolExecutor for EchoTool {
    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        Self::execute(req)
    }

    fn has_tool(&self, name: &str) -> bool {
        name == Self::NAME
    }

    fn tool_names(&self) -> Vec<&str> {
        vec![Self::NAME]
    }
}

/// A tool that adds two numbers.
///
/// Expects args in the form `{"a": N, "b": N}` and returns `{"result": N}`.
/// Returns an error outcome for malformed input.
#[derive(Debug, Default, Clone, Copy)]
struct AddTool;

impl AddTool {
    /// Tool name exposed to agents.
    const NAME: &'static str = "add";

    /// Execute the add tool.
    fn execute(req: &ToolCallRequest) -> ToolResult {
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

impl ToolExecutor for AddTool {
    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        Self::execute(req)
    }

    fn has_tool(&self, name: &str) -> bool {
        name == Self::NAME
    }

    fn tool_names(&self) -> Vec<&str> {
        vec![Self::NAME]
    }
}

/// A tool that executes shell commands.
///
/// Expects args in the form:
/// {
///   "command": "echo",
///   "args": ["Hello, World!"]
/// }
#[derive(Debug, Default, Clone, Copy)]
struct ShellTool;

impl ShellTool {
    /// Tool name exposed to agents.
    const NAME: &'static str = "shell";

    /// Execute a shell command.
    fn execute(req: &ToolCallRequest) -> ToolResult {
        // Validate command field
        let command = req
            .args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'command' field".to_string())
            .map_err(|message| ToolResult {
                call_id: req.call_id.clone(),
                tool_name: Self::NAME.to_string(),
                outcome: ToolOutcome::Error {
                    code: "INVALID_ARGS".to_string(),
                    message,
                },
            });

        if let Err(result) = command {
            return result;
        }

        let command = command.unwrap();
        // Parse optional arguments (if provided as array)
        let args = req
            .args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<&str>>()
            })
            .unwrap_or_default();

        // Build and execute command
        let mut cmd = std::process::Command::new(command);
        cmd.args(&args);

        match cmd.output() {
            Ok(output) => {
                let success = output.status.success();
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

                if success {
                    ToolResult {
                        call_id: req.call_id.clone(),
                        tool_name: Self::NAME.to_string(),
                        outcome: ToolOutcome::Success(serde_json::json!({
                            "stdout": stdout,
                            "stderr": stderr,
                        })),
                    }
                } else {
                    ToolResult {
                        call_id: req.call_id.clone(),
                        tool_name: Self::NAME.to_string(),
                        outcome: ToolOutcome::Error {
                            code: "COMMAND_ERROR".to_string(),
                            message: format!(
                                "command failed: {}",
                                stderr.trim().lines().next().unwrap_or("No error message")
                            ),
                        },
                    }
                }
            }
            Err(e) => ToolResult {
                call_id: req.call_id.clone(),
                tool_name: Self::NAME.to_string(),
                outcome: ToolOutcome::Error {
                    code: "SYSTEM_ERROR".to_string(),
                    message: e.to_string(),
                },
            },
        }
    }
}

impl ToolExecutor for ShellTool {
    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        Self::execute(req)
    }

    fn has_tool(&self, name: &str) -> bool {
        name == Self::NAME
    }

    fn tool_names(&self) -> Vec<&str> {
        vec![Self::NAME]
    }
}

/// A registry-based executor that dispatches to registered tools.
///
/// This replaces the previous hardcoded BuiltinToolExecutor. The registry
/// pattern allows adding new tools without changing the executor implementation.
#[derive(Default)]
pub struct BuiltinToolExecutor {
    registry: ToolRegistry,
}

impl BuiltinToolExecutor {
    /// Create an executor with the default tools (`echo`, `add`).
    pub fn new() -> Self {
        let mut registry = ToolRegistry::new();
        registry.register("echo".to_string(), Box::new(EchoTool));
        registry.register("add".to_string(), Box::new(AddTool));
        Self { registry }
    }

    /// Register a new tool by name and implementation.
    pub fn register(&mut self, name: String, executor: Box<dyn ToolExecutor>) {
        self.registry.register(name, executor);
    }

    /// Register shell tool if not already present.
    pub fn with_shell_tool(mut self) -> Self {
        if !self.registry.has_tool(ShellTool::NAME) {
            self.register(ShellTool::NAME.to_string(), Box::new(ShellTool));
        }
        self
    }
}

impl ToolExecutor for BuiltinToolExecutor {
    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        // If no tool is registered, return a standardized error.
        if let Some(result) = self.registry.execute(&req.tool_name, req) {
            return result;
        }

        ToolResult {
            call_id: req.call_id.clone(),
            tool_name: req.tool_name.clone(),
            outcome: ToolOutcome::Error {
                code: "UNKNOWN_TOOL".to_string(),
                message: format!("no tool registered with name '{}'", req.tool_name),
            },
        }
    }

    fn has_tool(&self, name: &str) -> bool {
        self.registry.has_tool(name)
    }

    fn tool_names(&self) -> Vec<&str> {
        self.registry.tool_names()
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

    // --- ToolRegistry tests --------------------------------------------------

    #[test]
    fn test_tool_registry_basic() {
        let mut registry = ToolRegistry::new();
        registry.register("newtool".to_string(), Box::new(EchoTool));
        assert!(registry.has_tool("newtool"));
        assert_eq!(registry.tool_names(), ["newtool"]);
        let result = registry
            .execute("newtool", &make_request("newtool", serde_json::json!({})))
            .unwrap();
        assert_eq!(result.outcome, ToolOutcome::Success(serde_json::json!({})));
    }

    #[test]
    fn test_tool_registry_empty() {
        let registry = ToolRegistry::new();
        assert!(!registry.has_tool("none"));
        assert!(registry.tool_names().is_empty());
        let result = registry.execute("none", &make_request("none", serde_json::json!({})));
        assert!(result.is_none());
    }

    #[test]
    fn test_tool_registry_with_defaults() {
        let mut exec = BuiltinToolExecutor::new();
        assert!(exec.has_tool("echo"));
        assert!(exec.has_tool("add"));
        assert!(!exec.has_tool("newtool"));

        exec.register("newtool".to_string(), Box::new(EchoTool));
        assert!(exec.has_tool("newtool"));
        assert_eq!(exec.tool_names().len(), 3);

        let result = exec.execute(&make_request("newtool", serde_json::json!({})));
        assert_eq!(result.outcome, ToolOutcome::Success(serde_json::json!({})));
    }

    // --- BuiltinToolExecutor tests -------------------------------------------

    #[test]
    fn test_builtin_executor_dispatches_echo() {
        let exec = BuiltinToolExecutor::new();
        let req = make_request("echo", serde_json::json!("ping"));
        let result = exec.execute(&req);
        assert_eq!(result.outcome, ToolOutcome::Success(serde_json::json!("ping")));
    }

    #[test]
    fn test_builtin_executor_dispatches_add() {
        let exec = BuiltinToolExecutor::new();
        let req = make_request("add", serde_json::json!({"a": 10, "b": 20}));
        let result = exec.execute(&req);
        assert_eq!(result.outcome, ToolOutcome::Success(serde_json::json!({"result": 30.0})));
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
            other => panic!("expected Error outcome, got {:?}", other),
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
        let mut exec = BuiltinToolExecutor::new();
        exec.register("custom".to_string(), Box::new(EchoTool));
        assert!(exec.has_tool("custom"));
        assert_eq!(exec.tool_names().len(), 3);
    }

    // --- ShellTool tests -----------------------------------------------------

    #[test]
    fn test_shell_tool_valid_command() {
        if cfg!(target_os = "windows") {
            // Windows echo (without /n)
            let req = make_request("shell", serde_json::json!({
                "command": "cmd",
                "args": ["/c", "echo", "Hello"]
            }));
            let result = ShellTool::execute(&req);

            match &result.outcome {
                ToolOutcome::Success(output) => {
                    let stdout = output.get("stdout").and_then(|v| v.as_str());
                    assert!(stdout.map(|s| s.trim()).unwrap_or("") == "Hello");
                }
                other => panic!("expected Success, got {:?}", other),
            }
        } else {
            let req = make_request("shell", serde_json::json!({
                "command": "echo",
                "args": ["Hello"]
            }));
            let result = ShellTool::execute(&req);

            match &result.outcome {
                ToolOutcome::Success(output) => {
                    let stdout = output.get("stdout").and_then(|v| v.as_str());
                    assert!(stdout.map(|s| s.trim()).unwrap_or("") == "Hello");
                }
                other => panic!("expected Success, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_shell_tool_missing_command() {
        let req = make_request("shell", serde_json::json!({
            "args": ["echo"]
        }));
        let result = ShellTool::execute(&req);

        match &result.outcome {
            ToolOutcome::Error { code, message } => {
                assert_eq!(code, "INVALID_ARGS");
                assert!(message.contains("missing 'command' field"));
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_shell_tool_invalid_command() {
        let req = make_request("shell", serde_json::json!({
            "command": "nonexistent_command",
            "args": []
        }));
        let result = ShellTool::execute(&req);

        match &result.outcome {
            ToolOutcome::Error { code, message } => {
                assert_eq!(code, "SYSTEM_ERROR");
                // Error message varies by platform
                assert!(!message.is_empty());
            }
            other => panic!("expected Error, got {:?}", other),
        }
    }
}
