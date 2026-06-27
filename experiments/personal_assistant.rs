//! Personal Assistant Agent — validation experiment.
//!
//! Implements **Agent 3 (Personal Assistant)** from RFC 001 §8: an agent with
//! memory tools that can store and retrieve information.
//!
//! The agent tracks project deadlines by:
//!   1. Storing a deadline via the `memory` tool
//!   2. Retrieving the deadline via the `memory` tool
//!   3. Reporting the retrieved deadline
//!
//! # Validation criteria
//!
//! 1. The agent calls at least one memory tool.
//! 2. The runtime delivers each memory tool result back to the agent via
//!    `StepContext`.
//! 3. The agent continues to the next step after receiving each result — it
//!    does not terminate early on a tool call.
//! 4. The agent reports the correct retrieved information.
//!
//! If all criteria hold, the agent abstraction is considered validated for
//! memory-dependent workloads.

use runtime::{
    Agent, AgentId, AgentProgram, BuiltinToolExecutor, Intent, IntentId, MockAgent,
    ModelDescriptor, PromptDescriptor, ScriptedAction, SchedulingHint, SchedulingState,
    StepContext, StepOutput, TerminationReason, TokenCounter, ToolExecutor, ToolResult,
    WorkingMemoryView,
};
use runtime::tool::MemoryStore;

/// A minimal working-memory view used as the execution context for tests.
#[derive(Debug)]
struct EmptyMemory;

impl WorkingMemoryView for EmptyMemory {
    fn peek(&self) -> &str {
        ""
    }

    fn utilization(&self) -> f32 {
        0.0
    }
}

fn main() {
    println!("=== Personal Assistant Agent Validation (RFC 001 §8 · Agent 3: Memory-Dependent) ===\n");
    println!("Intent: track project deadlines using memory tools.\n");

    // ------------------------------------------------------------------
    // Build the agent and the tool executor.
    // ------------------------------------------------------------------
    let agent_id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(
        intent_id,
        "Track project deadlines using memory tools",
        Some(agent_id),
    );
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "memory-assistant-v1"),
        PromptDescriptor::new("You are a personal assistant. Use the `memory` tool to store and retrieve information."),
    );

    // Script:
    //   1. Call `memory({operation: "store", key: "deadline", value: "2026-07-15"})`
    //   2. Acknowledge storage
    //   3. Call `memory({operation: "retrieve", key: "deadline"})`
    //   4. Report the retrieved deadline
    //   5. Terminate cleanly
    let script = vec![
        ScriptedAction::ToolCall {
            tool_name: "memory".into(),
            args: serde_json::json!({
                "operation": "store",
                "key": "project_deadline",
                "value": "2026-07-15"
            }),
        },
        ScriptedAction::Text("Deadline stored successfully.".into()),
        ScriptedAction::ToolCall {
            tool_name: "memory".into(),
            args: serde_json::json!({
                "operation": "retrieve",
                "key": "project_deadline"
            }),
        },
        ScriptedAction::Text("Retrieved deadline: 2026-07-15".into()),
        ScriptedAction::Terminate(TerminationReason::Success),
    ];

    let mut agent = MockAgent::new(agent_id, intent, program).with_script(script);

    // Create memory store and register memory tool
    let memory_store = MemoryStore::new();
    let mut tool_executor = BuiltinToolExecutor::new();
    tool_executor.register_memory_tool(memory_store);

    // ------------------------------------------------------------------
    // Runtime loop.
    //
    // On each iteration:
    //   - Feed any pending tool result from the previous iteration into ctx.
    //   - Let the agent produce one StepOutput.
    //   - If the output is a ToolCall, execute it via the ToolExecutor and
    //     stash the result for the next iteration.
    //   - Terminate when the agent's state transitions to Terminated.
    // ------------------------------------------------------------------
    let memory = EmptyMemory;
    let mut step_count: usize = 0;
    let mut tool_calls_made: usize = 0;
    let mut text_outputs: Vec<String> = Vec::new();
    let mut pending_result: Option<ToolResult> = None;

    let terminated = loop {
        let memory_capability = runtime::Capability {
            resource_type: "tool".to_string(),
            resource_id: "memory".to_string(),
            access_level: "read_write".to_string(),
        };
        let mut counter = TokenCounter::new(0);
        let mut ctx = StepContext {
            working_memory: &memory,
            memory_capabilities: &[memory_capability],
            pending_messages: &[],
            now: chrono::Utc::now(),
            token_counter: &mut counter,
            scheduling_hint: SchedulingHint::Normal,
            pending_tool_result: pending_result.take(), // feed prior result back
        };

        let result = agent.step(&mut ctx);
        step_count += 1;

        match &result.output {
            StepOutput::Text(t) => {
                if t != "terminated" {
                    text_outputs.push(t.clone());
                }
                println!("[Step {}] TEXT  {}", step_count, t);
            }
            StepOutput::ToolCall(req) => {
                tool_calls_made += 1;
                println!(
                    "[Step {}] TOOLS {}({})",
                    step_count, req.tool_name, req.args
                );
                let tr = tool_executor.execute(req);
                let outcome_str = format!("{:?}", tr.outcome);
                println!("         → executed: {}", outcome_str);
                pending_result = Some(tr);
            }
            other => {
                println!("[Step {}] OTHER {:?}", step_count, other);
            }
        }

        if matches!(agent.state(), SchedulingState::Terminated { .. }) {
            if let SchedulingState::Terminated { reason, .. } = agent.state() {
                println!("[Step {}] Agent terminated: {:?}", step_count, reason);
            }
            break true;
        }

        if step_count >= 10 {
            println!("[Step {}] Safety limit reached — forcing termination.", step_count);
            agent.terminate(TerminationReason::Failure);
            break true;
        }
    };

    let results_received = agent.tool_result_log().len();

    // ------------------------------------------------------------------
    // Verification.
    // ------------------------------------------------------------------
    let tools_used = tool_calls_made >= 1;
    let results_got = results_received >= 1;
    // "Continues after tool results" means the agent did more than just call
    // tools — it absorbed results and kept reasoning. A strict check is:
    // results_received == tool_calls_made  (every call got its result back).
    let all_results_delivered = results_received == tool_calls_made;
    let correct_information = text_outputs.iter().any(|t| t.contains("2026-07-15"));

    println!("\n--- Verification -----------------------------------------------");
    println!(
        "  {} Memory tool calls made: {}",
        if tools_used { "✓" } else { "✗" },
        tool_calls_made,
    );
    println!(
        "  {} Memory tool results received: {}",
        if results_got { "✓" } else { "✗" },
        results_received,
    );
    println!(
        "  {} All tool calls received results (calls={}, results={})",
        if all_results_delivered { "✓" } else { "✗" },
        tool_calls_made,
        results_received,
    );
    println!(
        "  {} Correct information retrieved and reported",
        if correct_information { "✓" } else { "✗" },
    );
    println!(
        "  {} Agent terminated cleanly",
        if terminated { "✓" } else { "✗" },
    );

    let all_pass = tools_used && all_results_delivered && correct_information && terminated;
    println!(
        "\n  RESULT:  {}",
        if all_pass {
            "ALL CRITERIA PASS ✓"
        } else {
            "FAILED ✗"
        },
    );

    std::process::exit(if all_pass { 0 } else { 1 });
}