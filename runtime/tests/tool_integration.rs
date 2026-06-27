//! End-to-end integration tests for the tool-call pipeline.
//!
//! These tests exercise the full cycle:
//!
//! 1. `MockAgent::step()` returns `StepOutput::ToolCall(req)`.
//! 2. The runtime (here, the test harness) dispatches `req` through a
//!    `ToolExecutor`.
//! 3. The resulting `ToolResult` is fed back on the next `step()` call via
//!    `StepContext::pending_tool_result`.
//! 4. The agent absorbs the result into its internal log.
//!
//! The tests deliberately avoid touching LLM-specific code — they validate
//! the *mechanism* (P1), not any particular tool policy.

use runtime::{
    Agent, AgentId, AgentProgram, BuiltinToolExecutor, Intent, IntentId, MockAgent, ModelDescriptor,
    PromptDescriptor, ScriptedAction, SchedulingHint, StepContext, StepOutput, TokenCounter,
    ToolExecutor, ToolOutcome, ToolResult, WorkingMemoryView,
};

/// Minimal working memory view for integration tests.
#[derive(Debug)]
struct TestMemory(String);

impl WorkingMemoryView for TestMemory {
    fn peek(&self) -> &str {
        &self.0
    }
    fn utilization(&self) -> f32 {
        0.0
    }
}

/// Build a fresh `StepContext` with an optional pending tool result.
fn make_context<'a>(
    mem: &'a TestMemory,
    counter: &'a mut TokenCounter,
    pending: Option<ToolResult>,
) -> StepContext<'a> {
    StepContext {
        working_memory: mem,
        memory_capabilities: &[],
        pending_messages: &[],
        now: chrono::Utc::now(),
        token_counter: counter,
        scheduling_hint: SchedulingHint::Normal,
        pending_tool_result: pending,
    }
}

/// Run one step of the agent, dispatching any tool call through the given
/// executor. Returns the step's result and, if a tool call was produced,
/// the `ToolResult` that the caller should feed back on the next call.
///
/// This simulates one iteration of a minimal runtime loop.
fn step_with_dispatch(
    agent: &mut MockAgent,
    executor: &dyn ToolExecutor,
    mem: &TestMemory,
    pending: Option<ToolResult>,
) -> (runtime::StepResult, Option<ToolResult>) {
    let mut counter = TokenCounter::new(0);
    let mut ctx = make_context(mem, &mut counter, pending);
    let result = agent.step(&mut ctx);

    let next_pending = if let StepOutput::ToolCall(ref req) = result.output {
        Some(executor.execute(req))
    } else {
        None
    };

    (result, next_pending)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_full_tool_call_cycle() {
    let executor = BuiltinToolExecutor::new();
    let mem = TestMemory(String::new());

    let id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(intent_id, "add two numbers", Some(id));
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "v1"),
        PromptDescriptor::new("calculator"),
    );
    let mut agent = MockAgent::new(id, intent, program).with_script(vec![
        ScriptedAction::ToolCall {
            tool_name: "add".to_string(),
            args: serde_json::json!({"a": 10, "b": 32}),
        },
        // A second action so the agent has something to produce while
        // absorbing the result from the first.
        ScriptedAction::Text("done".to_string()),
    ]);

    // Step 1: agent produces a ToolCall.
    let (r1, pending1) = step_with_dispatch(&mut agent, &executor, &mem, None);
    match &r1.output {
        StepOutput::ToolCall(req) => {
            assert_eq!(req.tool_name, "add");
            assert_eq!(req.args, serde_json::json!({"a": 10, "b": 32}));
        }
        other => panic!("expected ToolCall, got {:?}", other),
    }
    assert!(pending1.is_some());

    // Step 2: agent absorbs the result and produces the next action.
    let (r2, pending2) = step_with_dispatch(&mut agent, &executor, &mem, pending1);
    match &r2.output {
        StepOutput::Text(t) => assert_eq!(t, "done"),
        other => panic!("expected Text, got {:?}", other),
    }
    assert!(pending2.is_none());

    // The agent should have absorbed the tool result.
    assert_eq!(agent.tool_result_log().len(), 1);
    let logged = &agent.tool_result_log()[0];
    assert_eq!(logged.tool_name, "add");
    assert_eq!(
        logged.outcome,
        ToolOutcome::Success(serde_json::json!({"result": 42.0}))
    );
}

#[test]
fn test_multi_step_tool_workflow() {
    let executor = BuiltinToolExecutor::new();
    let mem = TestMemory(String::new());

    let id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(intent_id, "compute", Some(id));
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "v1"),
        PromptDescriptor::new("multi-tool agent"),
    );

    // The script interleaves tool calls with "absorb" slots. Each tool call
    // produces a result that must be consumed on the following step; we use
    // a Text action as the post-tool slot so the agent has somewhere to
    // deliver the result while also producing output.
    let mut agent = MockAgent::new(id, intent, program).with_script(vec![
        ScriptedAction::ToolCall {
            tool_name: "echo".to_string(),
            args: serde_json::json!({"message": "hello"}),
        },
        ScriptedAction::ToolCall {
            tool_name: "add".to_string(),
            args: serde_json::json!({"a": 100, "b": 200}),
        },
        ScriptedAction::Text("done".to_string()),
    ]);

    // Step 1: produce echo ToolCall.
    let (r1, pending1) = step_with_dispatch(&mut agent, &executor, &mem, None);
    match &r1.output {
        StepOutput::ToolCall(req) => assert_eq!(req.tool_name, "echo"),
        other => panic!("step 1: expected ToolCall, got {:?}", other),
    }

    // Step 2: absorb echo result, produce add ToolCall.
    let (r2, pending2) = step_with_dispatch(&mut agent, &executor, &mem, pending1);
    match &r2.output {
        StepOutput::ToolCall(req) => assert_eq!(req.tool_name, "add"),
        other => panic!("step 2: expected ToolCall, got {:?}", other),
    }

    // Step 3: absorb add result, produce "done".
    let (r3, pending3) = step_with_dispatch(&mut agent, &executor, &mem, pending2);
    match &r3.output {
        StepOutput::Text(t) => assert_eq!(t, "done"),
        other => panic!("step 3: expected Text, got {:?}", other),
    }
    assert!(pending3.is_none());

    // Both tool results should be in the log, in order.
    assert_eq!(agent.tool_result_log().len(), 2);

    assert_eq!(agent.tool_result_log()[0].tool_name, "echo");
    assert_eq!(
        agent.tool_result_log()[0].outcome,
        ToolOutcome::Success(serde_json::json!({"message": "hello"}))
    );

    assert_eq!(agent.tool_result_log()[1].tool_name, "add");
    assert_eq!(
        agent.tool_result_log()[1].outcome,
        ToolOutcome::Success(serde_json::json!({"result": 300.0}))
    );
}

#[test]
fn test_unknown_tool_returns_error_outcome() {
    let executor = BuiltinToolExecutor::new();
    let mem = TestMemory(String::new());

    let mut agent = MockAgent::default().with_script(vec![
        ScriptedAction::ToolCall {
            tool_name: "nonexistent".to_string(),
            args: serde_json::json!({}),
        },
        ScriptedAction::Text("done".to_string()),
    ]);

    let (r1, pending1) = step_with_dispatch(&mut agent, &executor, &mem, None);

    // The agent produced the ToolCall.
    assert!(matches!(r1.output, StepOutput::ToolCall(_)));
    assert!(pending1.is_some());

    // Deliver the result.
    let (_r2, _) = step_with_dispatch(&mut agent, &executor, &mem, pending1);

    // The logged result should carry an Error outcome.
    assert_eq!(agent.tool_result_log().len(), 1);
    match &agent.tool_result_log()[0].outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, "UNKNOWN_TOOL");
            assert!(message.contains("nonexistent"));
        }
        other => panic!("expected Error outcome, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Memory Tool Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod memory_tool_tests {
    use runtime::{ToolCallRequest, ToolOutcome, ToolExecutor, MemoryTool, MemoryStore};
    use serde_json::json;

    #[test]
    fn test_memory_tool_store_and_retrieve() {
        let store = MemoryStore::new();
        let tool = MemoryTool::new(store.clone());

        // Test store operation
        let store_req = ToolCallRequest {
            call_id: "test-store-1".to_string(),
            tool_name: "memory".to_string(),
            args: json!({
                "operation": "store",
                "key": "project_deadline",
                "value": "2026-07-15"
            }),
        };
        let result = tool.execute(&store_req);

        match &result.outcome {
            ToolOutcome::Success(value) => {
                assert_eq!(value.as_str().unwrap(), "Stored successfully");
            }
            _ => panic!("Expected success, got {:?}", result.outcome),
        }

        // Test retrieve operation
        let retrieve_req = ToolCallRequest {
            call_id: "test-retrieve-1".to_string(),
            tool_name: "memory".to_string(),
            args: json!({
                "operation": "retrieve",
                "key": "project_deadline"
            }),
        };
        let result = tool.execute(&retrieve_req);

        match &result.outcome {
            ToolOutcome::Success(value) => {
                assert_eq!(value.as_str().unwrap(), "2026-07-15");
            }
            _ => panic!("Expected success, got {:?}", result.outcome),
        }
    }

    #[test]
    fn test_memory_tool_key_not_found() {
        let store = MemoryStore::new();
        let tool = MemoryTool::new(store.clone());

        let retrieve_req = ToolCallRequest {
            call_id: "test-retrieve-2".to_string(),
            tool_name: "memory".to_string(),
            args: json!({
                "operation": "retrieve",
                "key": "nonexistent_key"
            }),
        };
        let result = tool.execute(&retrieve_req);

        match &result.outcome {
            ToolOutcome::Error { code, message: _ } => {
                assert_eq!(code, "KEY_NOT_FOUND");
            }
            _ => panic!("Expected error, got {:?}", result.outcome),
        }
    }
}
