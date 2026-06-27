//! Memory Tool — RFC-001 §8 validation agent 3 (Personal Assistant)
//!
//! Implements a memory tool that agents can use to store and retrieve information.
//! This tool demonstrates the memory operation flow:
//!   1. Agent calls memory tool via StepOutput::ToolCall
//!   2. Runtime executes tool via ToolExecutor
//!   3. Tool result returned to agent via StepContext::pending_tool_result

use crate::tool::{ToolExecutor, ToolResult, ToolOutcome};
use crate::ToolCallRequest;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A simple in-memory storage for demonstration purposes.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    data: Arc<Mutex<HashMap<String, Value>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: String, value: Value) {
        self.data.lock().unwrap().insert(key, value);
    }

    pub fn clear(&self) {
        self.data.lock().unwrap().clear();
    }
}

/// Memory Tool — allows agents to store and retrieve information.
#[derive(Debug)]
pub struct MemoryTool {
    store: MemoryStore,
}

impl MemoryTool {
    const TOOL_NAME: &'static str = "memory";

    pub fn new(store: MemoryStore) -> Self {
        Self { store }
    }
}

impl ToolExecutor for MemoryTool {

    fn execute(&self, req: &ToolCallRequest) -> ToolResult {
        let operation = req.args.get("operation").and_then(|v| v.as_str()).unwrap_or("");
        let key = req.args.get("key").and_then(|v| v.as_str()).unwrap_or("");

        match operation {
            "store" => {
                let value = req.args.get("value").cloned().unwrap_or(Value::Null);
                self.store.set(key.to_string(), value);
                ToolResult {
                    call_id: req.call_id.clone(),
                    tool_name: Self::TOOL_NAME.to_string(),
                    outcome: ToolOutcome::Success(Value::String("Stored successfully".to_string())),
                }
            }
            "retrieve" => {
                match self.store.get(key) {
                    Some(value) => ToolResult {
                        call_id: req.call_id.clone(),
                        tool_name: Self::TOOL_NAME.to_string(),
                        outcome: ToolOutcome::Success(value),
                    },
                    None => ToolResult {
                        call_id: req.call_id.clone(),
                        tool_name: Self::TOOL_NAME.to_string(),
                        outcome: ToolOutcome::Error {
                            code: "KEY_NOT_FOUND".to_string(),
                            message: format!("Key '{}' not found in memory", key),
                        },
                    },
                }
            }
            _ => ToolResult {
                call_id: req.call_id.clone(),
                tool_name: Self::TOOL_NAME.to_string(),
                outcome: ToolOutcome::Error {
                    code: "INVALID_OPERATION".to_string(),
                    message: format!("Invalid operation '{}'. Supported: store, retrieve", operation),
                },
            },
        }
    }

    fn has_tool(&self, name: &str) -> bool {
        name == Self::TOOL_NAME
    }

    fn tool_names(&self) -> Vec<&str> {
        vec![Self::TOOL_NAME]
    }
}
