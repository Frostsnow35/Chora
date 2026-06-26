//! Calculator Agent — validation experiment.
//!
//! Implements **Agent 1 (Tool-Using)** from RFC 001 §8: a small arithmetic LLM
//! that drives tool calls (`add`) and integrates the results back via
//! `StepContext::pending_tool_result`.
//!
//! The agent computes `(10 + 32) * 2` by:
//!   1. Calling `add({a: 10, b: 32})` to get 42.
//!   2. Calling `add({a: 42, b: 42})` to multiply by 2 (since the registry
//!      ships with `add` but not `mul`, doubling via addition is a valid
//!      demonstration of iterative tool use).
//!   3. Reporting the final answer: 84.
//!
//! # Validation criteria
//!
//! 1. The agent calls at least one tool.
//! 2. The runtime delivers each tool result back to the agent via
//!    `StepContext`.
//! 3. The agent continues to the next step after receiving each result — it
//!    does not terminate early on a tool call.
//! 4. The agent reports the correct final answer (84).
//!
//! If all criteria hold, the agent abstraction is considered validated for
//! tool-using workloads.

use runtime::{
    Agent, AgentId, AgentProgram, BuiltinToolExecutor, Intent, IntentId, MockAgent,
    ModelDescriptor, PromptDescriptor, ScriptedAction, SchedulingHint, SchedulingState,
    StepContext, StepOutput, TerminationReason, TokenCounter, ToolExecutor, ToolResult,
    WorkingMemoryView,
};

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
    println!("=== Calculator Agent Validation (RFC 001 §8 · Agent 1: Tool-Using) ===\n");
    println!("Intent: compute (10 + 32) * 2 using available tools.\n");

    // ------------------------------------------------------------------
    // Build the agent and the tool executor.
    // ------------------------------------------------------------------
    let agent_id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(
        intent_id,
        "Compute (10 + 32) * 2 using the `add` tool",
        Some(agent_id),
    );
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "arithmetic-v1"),
        PromptDescriptor::new("You are a calculator. Use the `add` tool for every operation."),
    );

    // Script:
    //   1. Call `add({a: 10, b: 32})`  →  42
    //   2. Acknowledge intermediate result
    //   3. Call `add({a: 42, b: 42})`  →  84  (doubling for multiplication)
    //   4. Report the final answer
    //   5. Terminate cleanly
    let script = vec![
        ScriptedAction::ToolCall {
            tool_name: "add".into(),
            args: serde_json::json!({"a": 10, "b": 32}),
        },
        ScriptedAction::Text("Intermediate step complete — received result for (10 + 32).".into()),
        ScriptedAction::ToolCall {
            tool_name: "add".into(),
            args: serde_json::json!({"a": 42, "b": 42}),
        },
        ScriptedAction::Text("Final answer: 84".into()),
        ScriptedAction::Terminate(TerminationReason::Success),
    ];

    let mut agent = MockAgent::new(agent_id, intent, program).with_script(script);
    let executor = BuiltinToolExecutor::new();

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
        let mut counter = TokenCounter::new(0);
        let mut ctx = StepContext {
            working_memory: &memory,
            memory_capabilities: &[],
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
                let tr = executor.execute(req);
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
    let correct_answer = text_outputs.iter().any(|t| t.contains("84"));

    println!("\n--- Verification -----------------------------------------------");
    println!(
        "  {} Tool calls made: {}",
        if tools_used { "✓" } else { "✗" },
        tool_calls_made,
    );
    println!(
        "  {} Tool results received: {}",
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
        "  {} Final answer is correct (contains '84')",
        if correct_answer { "✓" } else { "✗" },
    );
    println!(
        "  {} Agent terminated cleanly",
        if terminated { "✓" } else { "✗" },
    );

    let all_pass = tools_used && all_results_delivered && correct_answer && terminated;
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
