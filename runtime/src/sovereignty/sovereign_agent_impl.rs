//! Sovereign Agent Implementation — combines Kernel Space components.
//!
//! This module provides a reference implementation of the SovereignAgent trait
//! that integrates IntentCore, TrustMeter, and SovereigntyGate.

use std::sync::Arc;
use std::sync::Mutex;
use crate::{Agent, AgentId, Intent, SchedulingState, StepContext, StepResult};
use super::{SovereignAgent, SovereigntyLevel};
use super::trust_meter::TrustMeter;
use super::gate::SovereigntyGate;
use super::intent_core::IntentCore;

/// A sovereign agent implementation with Kernel Space / User Space separation.
///
/// This is a reference implementation that demonstrates how the sovereignty
/// system components work together. Real agents would implement their own
/// User Space logic (reasoning engine, tool dispatcher) while using this
/// Kernel Space structure.
pub struct SovereignAgentImpl {
    /// Base agent ID.
    id: AgentId,
    /// Base agent intent.
    intent: Intent,
    /// Scheduling state.
    state: SchedulingState,
    /// Kernel Space: Intent Core (immutable Constitution + mutable Execution Plan).
    #[allow(dead_code)] // Used by future User Space reasoning logic
    intent_core: Arc<Mutex<IntentCore>>,
    /// Kernel Space: Trust Meter (tracks trustworthiness).
    trust_meter: Arc<Mutex<TrustMeter>>,
    /// Kernel Space: Sovereignty Gate (enforces access control).
    sovereignty_gate: Arc<Mutex<SovereigntyGate>>,
}

impl SovereignAgentImpl {
    /// Create a new sovereign agent.
    pub fn new(
        id: AgentId,
        intent: Intent,
        intent_core: IntentCore,
        initial_trust_score: f64,
    ) -> Self {
        let trust_meter = Arc::new(Mutex::new(TrustMeter::new(
            initial_trust_score,
            crate::sovereignty::trust_meter::SovereigntyThresholds::default(),
        )));
        let sovereignty_gate = Arc::new(Mutex::new(SovereigntyGate::new(trust_meter.clone())));

        Self {
            id,
            intent,
            state: SchedulingState::Ready,
            intent_core: Arc::new(Mutex::new(intent_core)),
            trust_meter,
            sovereignty_gate,
        }
    }

    /// Record a trust event (called by User Space after sovereignty API calls).
    pub fn record_trust_event(&self, step_id: u64, behavior: crate::sovereignty::trust_meter::TrustBehavior) {
        // Record the event first
        {
            let mut meter = self.trust_meter.lock().unwrap();
            meter.record_event(step_id, behavior);
        }

        // Then update sovereignty gate level
        {
            let mut gate = self.sovereignty_gate.lock().unwrap();
            gate.update_level();
        }
    }

    /// Check if a sovereignty API call is allowed.
    pub fn check_sovereignty_access(&self, api: &crate::sovereignty::gate::SovereigntyApi) -> bool {
        let gate = self.sovereignty_gate.lock().unwrap();
        gate.check_access(api, 0).is_ok()
    }
}

impl Agent for SovereignAgentImpl {
    fn id(&self) -> AgentId {
        self.id
    }

    fn intent(&self) -> &Intent {
        &self.intent
    }

    fn state(&self) -> SchedulingState {
        self.state.clone()
    }

    fn step(&mut self, _ctx: &mut StepContext) -> StepResult {
        // Placeholder implementation — real agents would implement their own logic
        StepResult {
            status: crate::StepStatus::Success,
            output: crate::StepOutput::Text("Placeholder step output".to_string()),
            wants_yield: false,
            metrics: crate::StepMetrics {
                tokens_used: 0,
                wall_time_ms: 0,
            },
        }
    }

    fn receive(&mut self, _msg: crate::Message) -> Result<(), crate::DeliveryError> {
        // Placeholder implementation
        Ok(())
    }

    fn yield_now(&mut self) {
        self.state = SchedulingState::Suspended;
        // Record cooperative yield
        self.record_trust_event(0, crate::sovereignty::trust_meter::TrustBehavior::CooperativeYield);
    }

    fn resume(&mut self) {
        self.state = SchedulingState::Ready;
    }

    fn terminate(&mut self, reason: crate::TerminationReason) {
        self.state = SchedulingState::Terminated {
            reason,
            at: chrono::Utc::now(),
        };
    }
}

impl SovereignAgent for SovereignAgentImpl {
    fn sovereignty_level(&self) -> SovereigntyLevel {
        let gate = self.sovereignty_gate.lock().unwrap();
        gate.current_level()
    }

    fn trust_score(&self) -> f64 {
        let meter = self.trust_meter.lock().unwrap();
        meter.score()
    }
}