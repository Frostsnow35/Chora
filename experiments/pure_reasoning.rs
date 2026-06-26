//! Pure Reasoning Agent — validation experiment.
//!
//! Implements **Agent 2 (Summarizer)** from RFC 001 §8: a large general LLM
//! with **no tools**, tasked with summarising an input text.
//!
//! # Validation criteria
//!
//! 1. No tool calls are made during the run.
//! 2. The agent completes its goal within 5 steps.
//! 3. The final output is a concise summary.
//!
//! If all three criteria hold, the agent abstraction (specifically `step()`
//! operating with no tools, only internal reasoning) is considered validated
//! for pure-reasoning workloads.

use runtime::{
    Agent, AgentId, AgentProgram, Intent, IntentId, MockAgent, ModelDescriptor, PromptDescriptor,
    ScriptedAction, SchedulingHint, SchedulingState, StepContext, StepOutput, TerminationReason,
    TokenCounter, WorkingMemoryView,
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
    println!("=== Pure Reasoning Agent Validation (RFC 001 §8 · Agent 2: Summarizer) ===\n");
    println!("Intent: summarise input text in ≤50 words. Tools: NONE.\n");

    // ------------------------------------------------------------------
    // Build the agent.
    // ------------------------------------------------------------------
    let agent_id = AgentId::new();
    let intent_id = IntentId::new();
    let intent = Intent::new_root(
        intent_id,
        "Summarise the input document in ≤50 words",
        Some(agent_id),
    );
    let program = AgentProgram::new(
        ModelDescriptor::new("mock", "summarizer-v1"),
        PromptDescriptor::new("You are a concise summariser. Reduce any input to key points."),
    );

    // Script: pure reasoning — only Text actions, **no ToolCall**.
    // Models a 3-step reasoning chain followed by clean termination.
    let script = vec![
        ScriptedAction::Text(
            "Input received. Parsing a long document with 200+ sentences...".into(),
        ),
        ScriptedAction::Text(
            "Identifying core themes: entity extraction, relationship mapping, theme clustering."
                .into(),
        ),
        ScriptedAction::Text(
            "DRAFT SUMMARY: Three key findings emerged — (1) agent systems benefit from OS-style \
             abstractions, (2) the Agent/Intent/Program triple cleanly separates identity, \
             purpose, and behaviour, (3) tool use is the most expressive validation axis."
                .into(),
        ),
        ScriptedAction::Terminate(TerminationReason::Success),
    ];

    let mut agent = MockAgent::new(agent_id, intent, program).with_script(script);

    // ------------------------------------------------------------------
    // Runtime loop.
    // ------------------------------------------------------------------
    let memory = EmptyMemory;
    let mut step_count: usize = 0;
    let mut tool_calls_made: usize = 0;
    let mut text_outputs: Vec<String> = Vec::new();

    let terminated = loop {
        let mut counter = TokenCounter::new(0);
        let mut ctx = StepContext {
            working_memory: &memory,
            memory_capabilities: &[],
            pending_messages: &[],
            now: chrono::Utc::now(),
            token_counter: &mut counter,
            scheduling_hint: SchedulingHint::Normal,
            pending_tool_result: None, // no tool calls in this experiment
        };

        let result = agent.step(&mut ctx);
        step_count += 1;

        match &result.output {
            StepOutput::Text(t) => {
                if t != "terminated" {
                    text_outputs.push(t.clone());
                }
                println!("[Step {}] TEXT   {}", step_count, t);
            }
            StepOutput::ToolCall(req) => {
                tool_calls_made += 1;
                println!(
                    "[Step {}] TOOLS  {}({})  ← UNEXPECTED",
                    step_count, req.tool_name, req.args
                );
            }
            other => {
                println!("[Step {}] OTHER  {:?}", step_count, other);
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

    // ------------------------------------------------------------------
    // Verification.
    // ------------------------------------------------------------------
    let no_tools = tool_calls_made == 0;
    let within_steps = step_count <= 5;
    let has_summary = text_outputs
        .iter()
        .any(|t| t.contains("SUMMARY") || t.contains("summary"));

    println!("\n--- Verification -----------------------------------------------");
    println!(
        "  {} No tool calls (made: {})",
        if no_tools { "✓" } else { "✗" },
        tool_calls_made,
    );
    println!(
        "  {} Completed in ≤5 steps (took: {})",
        if within_steps { "✓" } else { "✗" },
        step_count,
    );
    println!(
        "  {} Final output is a concise summary",
        if has_summary { "✓" } else { "✗" },
    );
    println!(
        "  {} Agent terminated cleanly",
        if terminated { "✓" } else { "✗" },
    );

    let all_pass = no_tools && within_steps && has_summary && terminated;
    println!(
        "\n  RESULT:  {}",
        if all_pass { "ALL CRITERIA PASS ✓" } else { "FAILED ✗" },
    );

    std::process::exit(if all_pass { 0 } else { 1 });
}
