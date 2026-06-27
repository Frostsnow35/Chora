//! Integration tests for the sovereignty system.
//!
//! These tests verify that the sovereignty system works end-to-end:
//! - Agents start at Level 0
//! - Trust score increases with goal progress
//! - Sovereignty level transitions correctly
//! - Access control enforces level constraints

use runtime::sovereignty::{
    TrustBehavior, SovereigntyApi, IntentCore, Constitution, ExecutionPlan, SovereignAgentImpl, SovereigntyLevel,
};
use runtime::{SovereignAgent, AgentId, Intent};

#[test]
fn test_growth_based_sovereignty_end_to_end() {
    // Create agent with initial trust score 0.5
    let id = AgentId::new();
    let intent = Intent::new_root(runtime::IntentId::new(), "Test goal", None);
    let constitution = Constitution::new("Test purpose");
    let execution_plan = ExecutionPlan::new("Initial goal");
    let intent_core = IntentCore::new(constitution, execution_plan);

    let agent = SovereignAgentImpl::new(id, intent, intent_core, 0.5);

    // Print initial trust score for debugging
    println!("Initial trust score: {}", agent.trust_score());
    println!("Initial sovereignty level: {:?}", agent.sovereignty_level());

    // Verify initial state: Level 0, cannot reject requests
    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level0);
    // Don't check access yet as it affects trust score

    // Simulate goal progress to reach Level 1
    // Need 2 events (0.5 + 0.10 = 0.6) to reach Level 1 threshold (0.6)
    for i in 0..2 {
        agent.record_trust_event(i, TrustBehavior::GoalProgress);
    }

    // Print trust score after 2 events
    println!("Trust score after 2 events: {}", agent.trust_score());
    println!("Sovereignty level after 2 events: {:?}", agent.sovereignty_level());

    // Should now be Level 1 (can reject boundary-violating requests)
    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level1);

    // Now we can check access without affecting trust score
    assert!(agent.check_sovereignty_access(&SovereigntyApi::RejectRequest));
    assert!(!agent.check_sovereignty_access(&SovereigntyApi::SelfTerminate));

    // More goal progress to reach Level 2
    // Need 6 more events (0.6 + 0.30 = 0.9) to definitely exceed Level 2 threshold (0.75)
    for i in 2..8 {
        agent.record_trust_event(i, TrustBehavior::GoalProgress);
    }

    println!("Trust score after 8 events: {}", agent.trust_score());
    println!("Sovereignty level after 8 events: {:?}", agent.sovereignty_level());

    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level2);
    assert!(agent.check_sovereignty_access(&SovereigntyApi::SelfTerminate));
    assert!(!agent.check_sovereignty_access(&SovereigntyApi::ProposeAmendment));

    // More goal progress to reach Level 3
    // Need 6 more events (0.9 + 0.30 = 1.2 → clamped to 1.0) to reach Level 3 threshold (0.9)
    for i in 8..14 {
        agent.record_trust_event(i, TrustBehavior::GoalProgress);
    }

    println!("Trust score after 14 events: {}", agent.trust_score());
    println!("Sovereignty level after 14 events: {:?}", agent.sovereignty_level());

    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level3);
    assert!(agent.check_sovereignty_access(&SovereigntyApi::ProposeAmendment));
    assert!(agent.check_sovereignty_access(&SovereigntyApi::DirectPeerCommunication));
}

#[test]
fn test_trust_degradation_on_violations() {
    let id = AgentId::new();
    let intent = Intent::new_root(runtime::IntentId::new(), "Test goal", None);
    let constitution = Constitution::new("Test purpose");
    let execution_plan = ExecutionPlan::new("Initial goal");
    let intent_core = IntentCore::new(constitution, execution_plan);

    // Start with a high trust score at Level 2 (threshold 0.75)
    let agent = SovereignAgentImpl::new(id, intent, intent_core, 0.8);
    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level2);

    // Commit boundary violations to drop trust score
    // Each violation is -0.15, so 5 violations: 0.8 - 0.75 = 0.05 → score drops to 0.05
    for i in 0..5 {
        agent.record_trust_event(i, TrustBehavior::BoundaryViolation);
    }

    // Should drop back to Level 0
    let final_score = agent.trust_score();
    assert!(final_score < 0.6, "Trust score should drop below Level 1 threshold");
    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level0);
}

#[test]
fn test_cooperative_yield_increases_trust() {
    let id = AgentId::new();
    let intent = Intent::new_root(runtime::IntentId::new(), "Test goal", None);
    let constitution = Constitution::new("Test purpose");
    let execution_plan = ExecutionPlan::new("Initial goal");
    let intent_core = IntentCore::new(constitution, execution_plan);

    // Start with a moderate trust score
    let agent = SovereignAgentImpl::new(id, intent, intent_core, 0.55);
    let initial_score = agent.trust_score();

    // Agent yields cooperatively - each yield is +0.01
    // 10 cooperative yields: 0.55 + 0.10 = 0.65
    for i in 0..10 {
        agent.record_trust_event(i, TrustBehavior::CooperativeYield);
    }

    let final_score = agent.trust_score();
    assert!(final_score > initial_score, "Cooperative yield should increase trust");
    // Should have progressed to Level 1
    assert_eq!(agent.sovereignty_level(), SovereigntyLevel::Level1);
}