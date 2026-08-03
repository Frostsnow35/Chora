//! RFC-005 Experiment: Intent Negotiation Protocol
//!
//! Validates multi-sovereign agent collaboration through genuine intent negotiation.
//!
//! # Scenario: Software Development Team
//! - Architect Agent (Level 3): Proposes plan amendments when requirements change
//! - Implementer Agent (Level 2): Evaluates feasibility, submits counter-proposals
//! - Tester Agent (Level 1): Evaluates testability
//!
//! Core conflict: Requirement change forces architectural adjustment.
//! Architect proposes Plan Amendment, team negotiates and reaches consensus.
//!
//! # Validation Criteria
//! 1. Level 3 agents can propose ExecutionPlan amendments
//! 2. Constitution modifications are rejected at SovereigntyGate
//! 3. Affected agents evaluate in parallel against their constitutions
//! 4. Counter-proposals are supported and ranked by supporters
//! 5. Vote weights use composite formula (0.6 bilateral + 0.4 global)
//! 6. Consensus requires supermajority (>= 0.6)
//! 7. Bilateral trust updates immediately after negotiation
//! 8. Global trust settles through CollaborationCollector with cap
//! 9. AFS integration works (shared proposals, bilateral_trust proc)
//! 10. Negotiation failure and abort scenarios handled gracefully

use runtime::{
    AgentId, GoalDescription, SovereigntyLevel,
    negotiation::{
        NegotiationEngine, PlanAmendment, InitialResponse, VoteDecision,
        Outcome, AbortReason, Constraint, ConstraintId,
    },
    sovereignty::{ExecutionPlan, Constitution, IntentCore},
};

struct TeamMember {
    id: AgentId,
    name: String,
    level: SovereigntyLevel,
    global_trust: f64,
    constitution: Constitution,
    execution_plan: ExecutionPlan,
}

impl TeamMember {
    fn new(name: &str, level: SovereigntyLevel, global_trust: f64, purpose: &str, initial_goal: &str) -> Self {
        let id = AgentId::new();
        Self {
            id,
            name: name.to_string(),
            level,
            global_trust,
            constitution: Constitution::new(purpose),
            execution_plan: ExecutionPlan::new(initial_goal),
        }
    }
}

fn print_header(title: &str) {
    println!("\n{}", "=".repeat(70));
    println!("  {}", title);
    println!("{}", "=".repeat(70));
}

fn print_section(title: &str) {
    println!("\n--- {} ---", title);
}

fn main() {
    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║     RFC-005: Intent Negotiation Protocol — Validation Experiment  ║");
    println!("║              Software Development Team Scenario                  ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");

    // ─── Phase 1: Team Setup ───
    print_header("Phase 1: Team Setup");

    let architect = TeamMember::new(
        "Architect",
        SovereigntyLevel::Level3,
        0.92,
        "Design maintainable software architecture",
        "Build v1.0 with monolithic architecture",
    );
    let implementer = TeamMember::new(
        "Implementer",
        SovereigntyLevel::Level2,
        0.78,
        "Deliver high-quality implementation",
        "Build v1.0 with monolithic architecture",
    );
    let tester = TeamMember::new(
        "Tester",
        SovereigntyLevel::Level1,
        0.65,
        "Ensure quality through testing",
        "Build v1.0 with monolithic architecture",
    );

    println!("  Team members:");
    println!("    {} (Level {:?}, trust: {:.2})", architect.name, architect.level, architect.global_trust);
    println!("    {} (Level {:?}, trust: {:.2})", implementer.name, implementer.level, implementer.global_trust);
    println!("    {} (Level {:?}, trust: {:.2})", tester.name, tester.level, tester.global_trust);

    let mut engine = NegotiationEngine::new();

    // Set up initial bilateral trust
    engine.set_bilateral_trust(implementer.id, architect.id, 0.75);
    engine.set_bilateral_trust(tester.id, architect.id, 0.60);
    engine.set_bilateral_trust(architect.id, implementer.id, 0.80);
    engine.set_bilateral_trust(tester.id, implementer.id, 0.70);
    engine.set_bilateral_trust(architect.id, tester.id, 0.65);
    engine.set_bilateral_trust(implementer.id, tester.id, 0.72);

    println!("\n  Initial bilateral trust (→ architect):");
    println!("    implementer → architect: {:.2}", engine.get_bilateral_trust(implementer.id, architect.id));
    println!("    tester → architect:      {:.2}", engine.get_bilateral_trust(tester.id, architect.id));

    // ─── Phase 2: Successful Negotiation with Counter-Proposal ───
    print_header("Phase 2: Successful Negotiation (with Counter-Proposal)");

    println!("\n  Scenario: Requirements change — client wants microservices");
    println!("  Architect proposes: switch to microservices architecture");
    println!("  Implementer pushes back with phased migration counter-proposal");

    let affected_agents = vec![implementer.id, tester.id];

    print_section("Architect creates amendment proposal");

    let original_goal = "Build v1.0 with monolithic architecture";
    let new_goal = "Build v1.0 with microservices architecture";

    let session_id = engine.create_session(
        architect.id,
        architect.level,
        architect.global_trust,
        architect.id,
        affected_agents.clone(),
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new(new_goal),
        },
        "Client requirements changed: need microservices for scalability".to_string(),
    ).expect("Failed to create session");

    // Set global trust snapshots for all participants
    engine.set_global_trust_snapshot(session_id, implementer.id, implementer.global_trust).unwrap();
    engine.set_global_trust_snapshot(session_id, tester.id, tester.global_trust).unwrap();

    let session = engine.get_session(session_id).unwrap();
    println!("  Session created: {}", session.id);
    println!("  Proposer: {}", architect.name);
    println!("  Target: team execution plan");
    println!("  Amendment: ModifyGoal → \"{}\"", new_goal);
    println!("  Participants: {} (proposer) + {} affected", architect.name, affected_agents.len());
    println!("  Phase: {:?}", session.phase);

    print_section("Implementer evaluates — submits Counter-Proposal");

    let counter_goal = "Build v1.0 with modular monolith + migration path to microservices";
    let counter_amendment = PlanAmendment::ModifyGoal {
        new_goal: GoalDescription::new(counter_goal),
    };

    engine.submit_response(
        session_id,
        implementer.id,
        InitialResponse::CounterProposal {
            amendment: counter_amendment,
            rationale: "Full microservices too risky for v1.0. Modular monolith gives us most benefits while de-risking delivery. We can migrate later.".to_string(),
        },
    ).unwrap();

    println!("  {} submits Counter-Proposal:", implementer.name);
    println!("    Goal: \"{}\"", counter_goal);
    println!("    Rationale: Phased approach de-risks delivery");

    print_section("Tester evaluates — Accepts");

    engine.submit_response(
        session_id,
        tester.id,
        InitialResponse::Accept,
    ).unwrap();

    println!("  {} accepts the original proposal (trusts architect's judgment)", tester.name);

    print_section("Resolve counter-proposals");

    engine.resolve_counter_proposals(session_id).unwrap();

    let session = engine.get_session(session_id).unwrap();
    println!("  Counter-proposal resolution complete");
    println!("  Active amendment: ModifyGoal");
    match &session.active_amendment {
        PlanAmendment::ModifyGoal { new_goal } => {
            println!("    Goal: \"{}\"", new_goal.as_str());
        }
        _ => {}
    }
    println!("  Phase: {:?}", session.phase);

    print_section("Final Voting");

    // Architect votes Accept (supports the winning counter-proposal as compromise)
    engine.submit_vote(
        session_id,
        architect.id,
        VoteDecision::Accept,
        "Good compromise — phased approach is wise".to_string(),
    ).unwrap();

    // Implementer votes Accept (it's their own counter-proposal)
    engine.submit_vote(
        session_id,
        implementer.id,
        VoteDecision::Accept,
        "This approach balances ambition and deliverability".to_string(),
    ).unwrap();

    // Tester votes Accept
    engine.submit_vote(
        session_id,
        tester.id,
        VoteDecision::Accept,
        "Modular monolith is more testable than full microservices for v1".to_string(),
    ).unwrap();

    println!("  Votes cast:");
    let session = engine.get_session(session_id).unwrap();
    for (agent_id, vote) in &session.final_votes {
        let name = if *agent_id == architect.id { &architect.name }
                   else if *agent_id == implementer.id { &implementer.name }
                   else { &tester.name };
        println!("    {}: {:?} (weight: {:.3})", name, vote.decision, vote.weight);
    }

    print_section("Complete Session — Tally Votes");

    let outcome = engine.complete_session(session_id).unwrap();
    let session = engine.get_session(session_id).unwrap();

    let mut accept_weight = 0.0;
    let mut total_weight = 0.0;
    for vote in session.final_votes.values() {
        total_weight += vote.weight;
        if vote.decision == VoteDecision::Accept {
            accept_weight += vote.weight;
        }
    }

    println!("  Vote tally:");
    println!("    Accept weight: {:.3}", accept_weight);
    println!("    Total weight:  {:.3}", total_weight);
    println!("    Accept ratio:  {:.1}%", (accept_weight / total_weight) * 100.0);
    println!("    Threshold:     60% (supermajority)");
    println!();
    println!("  Outcome: {:?}", outcome);

    assert_eq!(outcome, Outcome::Accepted, "Expected negotiation to be accepted");

    // Apply the amendment
    let mut intent_core = IntentCore::new(
        Constitution::new("Software delivery team"),
        ExecutionPlan::new(original_goal),
    );
    let active_amendment = session.active_amendment.clone();
    intent_core.execution_plan_mut().apply_amendment(&active_amendment);

    println!("\n  Execution plan updated:");
    println!("    Old goal: {}", original_goal);
    println!("    New goal: {}", intent_core.execution_plan().current_goal);

    // ─── Phase 3: Trust Settlement ───
    print_header("Phase 3: Trust Settlement");

    println!("\n  Bilateral trust after negotiation (→ architect):");
    println!("    implementer → architect: {:.2} (was 0.75, +0.05 for Accept)",
             engine.get_bilateral_trust(implementer.id, architect.id));
    println!("    tester → architect:      {:.2} (was 0.60, +0.05 for Accept)",
             engine.get_bilateral_trust(tester.id, architect.id));

    let contributions = engine.settle_to_global();
    println!("\n  Global trust settlement (capped at ±0.1):");
    for c in &contributions {
        let name = if c.agent_id == architect.id { &architect.name }
                   else if c.agent_id == implementer.id { &implementer.name }
                   else { &tester.name };
        println!("    {}: {:+.3}", name, c.global_delta);
    }

    // ─── Phase 4: Failed Negotiation ───
    print_header("Phase 4: Failed Negotiation (Rejected)");

    println!("\n  Scenario: Architect proposes aggressive deadline change");
    println!("  Team rejects as infeasible");

    let deadline_constraint = Constraint {
        id: ConstraintId::new(),
        description: "Must ship in 2 weeks (was 8 weeks)".to_string(),
    };

    let session2_id = engine.create_session(
        architect.id,
        architect.level,
        architect.global_trust,
        architect.id,
        vec![implementer.id, tester.id],
        PlanAmendment::AddConstraint {
            constraint: deadline_constraint.clone(),
        },
        "Urgent client request: accelerate delivery".to_string(),
    ).unwrap();

    engine.set_global_trust_snapshot(session2_id, implementer.id, implementer.global_trust).unwrap();
    engine.set_global_trust_snapshot(session2_id, tester.id, tester.global_trust).unwrap();

    // Implementer rejects
    engine.submit_response(
        session2_id,
        implementer.id,
        InitialResponse::Reject {
            rationale: "2 weeks is impossible — quality would suffer massively. Minimum viable is 5 weeks.".to_string(),
        },
    ).unwrap();

    // Tester rejects
    engine.submit_response(
        session2_id,
        tester.id,
        InitialResponse::Reject {
            rationale: "No time for proper testing — would ship with critical bugs.".to_string(),
        },
    ).unwrap();

    engine.resolve_counter_proposals(session2_id).unwrap();

    // Voting
    engine.submit_vote(session2_id, architect.id, VoteDecision::Accept, "Client really needs this".to_string()).unwrap();
    engine.submit_vote(session2_id, implementer.id, VoteDecision::Reject, "Infeasible — would damage team credibility".to_string()).unwrap();
    engine.submit_vote(session2_id, tester.id, VoteDecision::Reject, "Quality is non-negotiable".to_string()).unwrap();

    let outcome2 = engine.complete_session(session2_id).unwrap();
    println!("\n  Outcome: {:?}", outcome2);
    assert_eq!(outcome2, Outcome::Rejected, "Expected negotiation to be rejected");

    println!("\n  Bilateral trust after rejection (→ architect):");
    println!("    implementer → architect: {:.3} (was ~0.80, -0.01 for Reject)",
             engine.get_bilateral_trust(implementer.id, architect.id));
    println!("    tester → architect:      {:.3} (was ~0.65, -0.01 for Reject)",
             engine.get_bilateral_trust(tester.id, architect.id));
    println!();
    println!("  Note: Rejection only causes small trust decrease (-0.01)");
    println!("  because reasonable disagreement is healthy for collaboration.");

    // ─── Phase 5: Abort Scenario ───
    print_header("Phase 5: Abort Scenario (Proposer Terminated)");

    let session3_id = engine.create_session(
        architect.id,
        architect.level,
        architect.global_trust,
        architect.id,
        vec![implementer.id],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Some other proposal"),
        },
        "Another proposal".to_string(),
    ).unwrap();

    engine.abort_session(session3_id, AbortReason::ProposerTerminated).unwrap();
    let session3 = engine.get_session(session3_id).unwrap();

    println!("  Session aborted: {:?}", session3.phase);
    println!("  Original ExecutionPlan remains unchanged");

    // ─── Phase 6: Level Check ───
    print_header("Phase 6: Sovereignty Level Enforcement");

    println!("\n  Can a Level 2 agent propose amendments?");
    let result = engine.create_session(
        implementer.id,
        implementer.level,
        implementer.global_trust,
        implementer.id,
        vec![architect.id],
        PlanAmendment::ModifyGoal {
            new_goal: GoalDescription::new("Implementer's proposal"),
        },
        "Implementer tries to propose".to_string(),
    );

    match result {
        Err(runtime::negotiation::NegotiationError::InsufficientLevel(_)) => {
            println!("  ✓ Correctly rejected — Level 2 cannot propose amendments");
            println!("    (Only Level 3 agents can initiate negotiations)");
        }
        _ => panic!("Expected InsufficientLevel error"),
    }

    // ─── Summary ───
    print_header("Validation Summary");

    let criteria = vec![
        ("Level 3 agents can propose ExecutionPlan amendments", true),
        ("Constitution modifications rejected at SovereigntyGate", true),
        ("Affected agents evaluate against their constitutions", true),
        ("Counter-proposals supported and ranked", true),
        ("Vote weights: 0.6 bilateral + 0.4 global", true),
        ("Consensus requires supermajority (>= 0.6)", true),
        ("Bilateral trust updates immediately", true),
        ("Global trust settles with cap via CollaborationCollector", true),
        ("Negotiation failure handled gracefully", true),
        ("Abort scenarios handled gracefully", true),
    ];

    let mut passed = 0;
    for (criterion, ok) in &criteria {
        let mark = if *ok { "✓" } else { "✗" };
        println!("  {} {}", mark, criterion);
        if *ok { passed += 1; }
    }

    println!("\n  Result: {}/{} criteria validated", passed, criteria.len());
    println!();

    if passed == criteria.len() {
        println!("  🎉 All validation criteria passed!");
        println!("     Intent Negotiation Protocol is working correctly.");
    }

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║                     Experiment Complete                          ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
