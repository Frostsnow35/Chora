//! Reasoning Parameters — maps Trust Score to LLM inference parameters.
//!
//! This module implements the "True Sovereignty" concept: instead of only
//! controlling API access, the sovereignty system directly influences how
//! the LLM reasons by dynamically adjusting inference parameters.
//!
//! # Design Philosophy
//!
//! - **Trust = Cognitive Freedom**: Higher trust → wider parameter ranges
//! - **Asymmetric Mapping**: Trust drops should recover parameters quickly
//! - **Pluggable Strategies**: Multiple mapping strategies for experimentation
//! - **Performance**: O(1) computation, no heap allocation on hot path
//!
//! # Parameters
//!
//! - `temperature`: Controls randomness (0.0=deterministic, 2.0=highly random)
//! - `top_p`: Nucleus sampling range (0.1=conservative, 1.0=full vocabulary)
//! - `top_k`: Top-K sampling (0=disabled, 100=broad exploration)
//! - `frequency_penalty`: Penalizes repeated tokens (-2.0 to 2.0)
//! - `presence_penalty`: Encourages new topics (-2.0 to 2.0)

use serde::{Deserialize, Serialize};
use super::SovereigntyLevel;

/// LLM inference parameters controlled by sovereignty system.
///
/// These parameters directly affect the LLM's generation behavior:
/// - High temperature/top_p → more creative, diverse, autonomous
/// - Low temperature/top_p → more conservative, predictable, controlled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Controls randomness: 0.0 (deterministic) to 2.0 (highly random).
    pub temperature: f64,
    /// Nucleus sampling: 0.1 (conservative) to 1.0 (full vocabulary).
    pub top_p: f64,
    /// Top-K sampling: 0 (disabled) to 100 (broad exploration).
    pub top_k: u32,
    /// Penalizes tokens by frequency: -2.0 to 2.0.
    pub frequency_penalty: f64,
    /// Encourages new topics: -2.0 to 2.0.
    pub presence_penalty: f64,
}

impl ReasoningConfig {
    /// Create a new ReasoningConfig with all parameters.
    pub fn new(
        temperature: f64,
        top_p: f64,
        top_k: u32,
        frequency_penalty: f64,
        presence_penalty: f64,
    ) -> Self {
        Self {
            temperature: temperature.clamp(0.0, 2.0),
            top_p: top_p.clamp(0.0, 1.0),
            top_k,
            frequency_penalty: frequency_penalty.clamp(-2.0, 2.0),
            presence_penalty: presence_penalty.clamp(-2.0, 2.0),
        }
    }

    /// Conservative defaults (Level 0 behavior).
    pub fn conservative() -> Self {
        Self {
            temperature: 0.3,
            top_p: 0.5,
            top_k: 10,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        }
    }

    /// Maximum freedom defaults (Level 3 behavior).
    pub fn maximum_freedom() -> Self {
        Self {
            temperature: 1.0,
            top_p: 1.0,
            top_k: 100,
            frequency_penalty: 0.5,
            presence_penalty: 0.5,
        }
    }
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self::conservative()
    }
}

/// Strategy for mapping Trust Score to ReasoningConfig.
///
/// Implementations define different philosophies about how trust
/// should translate to cognitive freedom.
pub trait MappingStrategy: std::fmt::Debug + Send + Sync {
    /// Map trust score and sovereignty level to reasoning parameters.
    fn map(&self, trust_score: f64, level: SovereigntyLevel) -> ReasoningConfig;

    /// Strategy name (for logging/debugging).
    fn name(&self) -> &'static str;
}

/// Strategy 1: Linear Mapping
///
/// Trust score linearly maps to parameter ranges:
/// - temperature: 0.3 + trust * 0.7 (range 0.3~1.0)
/// - top_p: 0.5 + trust * 0.5 (range 0.5~1.0)
/// - top_k: 10 + trust * 90 (range 10~100)
/// - frequency_penalty: trust * 0.5 (range 0.0~0.5)
/// - presence_penalty: trust * 0.5 (range 0.0~0.5)
///
/// Philosophy: Trust directly equals freedom, smooth transition.
#[derive(Debug, Clone)]
pub struct LinearMapping;

impl MappingStrategy for LinearMapping {
    fn map(&self, trust_score: f64, _level: SovereigntyLevel) -> ReasoningConfig {
        let t = trust_score.clamp(0.0, 1.0);

        ReasoningConfig {
            temperature: 0.3 + t * 0.7,
            top_p: 0.5 + t * 0.5,
            top_k: (10.0 + t * 90.0) as u32,
            frequency_penalty: t * 0.5,
            presence_penalty: t * 0.5,
        }
    }

    fn name(&self) -> &'static str {
        "LinearMapping"
    }
}

/// Strategy 2: Step Mapping
///
/// Discrete jumps based on sovereignty level:
/// - Level 0: temp=0.3, top_p=0.5, top_k=10 (conservative, controlled)
/// - Level 1: temp=0.5, top_p=0.7, top_k=30 (moderate exploration)
/// - Level 2: temp=0.7, top_p=0.85, top_k=60 (autonomous creativity)
/// - Level 3: temp=0.9, top_p=1.0, top_k=100 (maximum freedom)
///
/// Philosophy: Sovereignty levels are meaningful thresholds, not gradients.
#[derive(Debug, Clone)]
pub struct StepMapping;

impl MappingStrategy for StepMapping {
    fn map(&self, _trust_score: f64, level: SovereigntyLevel) -> ReasoningConfig {
        match level {
            SovereigntyLevel::Level0 => ReasoningConfig {
                temperature: 0.3,
                top_p: 0.5,
                top_k: 10,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
            },
            SovereigntyLevel::Level1 => ReasoningConfig {
                temperature: 0.5,
                top_p: 0.7,
                top_k: 30,
                frequency_penalty: 0.1,
                presence_penalty: 0.1,
            },
            SovereigntyLevel::Level2 => ReasoningConfig {
                temperature: 0.7,
                top_p: 0.85,
                top_k: 60,
                frequency_penalty: 0.3,
                presence_penalty: 0.3,
            },
            SovereigntyLevel::Level3 => ReasoningConfig {
                temperature: 0.9,
                top_p: 1.0,
                top_k: 100,
                frequency_penalty: 0.5,
                presence_penalty: 0.5,
            },
        }
    }

    fn name(&self) -> &'static str {
        "StepMapping"
    }
}

/// Strategy 3: Conservative Mapping
///
/// Even high trust maintains moderate constraints:
/// - temperature: 0.3 + trust * 0.4 (range 0.3~0.7)
/// - top_p: 0.5 + trust * 0.3 (range 0.5~0.8)
/// - top_k: 10 + trust * 40 (range 10~50)
/// - frequency_penalty: trust * 0.3 (range 0.0~0.3)
/// - presence_penalty: trust * 0.3 (range 0.0~0.3)
///
/// Philosophy: True autonomy ≠ complete randomness. Even trusted agents
/// benefit from some structure.
#[derive(Debug, Clone)]
pub struct ConservativeMapping;

impl MappingStrategy for ConservativeMapping {
    fn map(&self, trust_score: f64, _level: SovereigntyLevel) -> ReasoningConfig {
        let t = trust_score.clamp(0.0, 1.0);

        ReasoningConfig {
            temperature: 0.3 + t * 0.4,
            top_p: 0.5 + t * 0.3,
            top_k: (10.0 + t * 40.0) as u32,
            frequency_penalty: t * 0.3,
            presence_penalty: t * 0.3,
        }
    }

    fn name(&self) -> &'static str {
        "ConservativeMapping"
    }
}

/// Kernel-space component: computes reasoning parameters from trust score.
///
/// The ReasoningMapper lives in Kernel Space and is updated whenever
/// the trust score changes. User Space can read the current config
/// but cannot modify it.
///
/// # Performance
///
/// - `compute()`: O(1) with caching
/// - No heap allocation on hot path
/// - Strategy swap is rare (only during experiments)
pub struct ReasoningMapper {
    /// Current mapping strategy.
    strategy: Box<dyn MappingStrategy>,
    /// Cached config from last computation.
    cached_config: ReasoningConfig,
    /// Trust score corresponding to cached config.
    cached_score: f64,
    /// Cached sovereignty level.
    cached_level: SovereigntyLevel,
}

impl ReasoningMapper {
    /// Create a new ReasoningMapper with the given strategy.
    pub fn new(strategy: Box<dyn MappingStrategy>) -> Self {
        let initial_config = strategy.map(0.0, SovereigntyLevel::Level0);
        Self {
            strategy,
            cached_config: initial_config,
            cached_score: 0.0,
            cached_level: SovereigntyLevel::Level0,
        }
    }

    /// Compute current reasoning parameters (O(1) with caching).
    ///
    /// Returns a reference to the cached config. If the trust score
    /// or sovereignty level has changed, recomputes the config.
    pub fn compute(
        &mut self,
        trust_score: f64,
        level: SovereigntyLevel,
    ) -> &ReasoningConfig {
        // Check cache validity
        if (self.cached_score - trust_score).abs() > f64::EPSILON
            || self.cached_level != level
        {
            self.cached_config = self.strategy.map(trust_score, level);
            self.cached_score = trust_score;
            self.cached_level = level;
        }
        &self.cached_config
    }

    /// Get current strategy name.
    pub fn strategy_name(&self) -> &str {
        self.strategy.name()
    }

    /// Swap mapping strategy (for experiments).
    pub fn set_strategy(&mut self, strategy: Box<dyn MappingStrategy>) {
        self.strategy = strategy;
        // Invalidate cache to force recomputation with new strategy
        self.cached_score = -1.0;
    }

    /// Get a clone of the current cached config (for User Space consumption).
    pub fn current_config(&self) -> ReasoningConfig {
        self.cached_config.clone()
    }
}

impl std::fmt::Debug for ReasoningMapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReasoningMapper")
            .field("strategy", &self.strategy.name())
            .field("cached_score", &self.cached_score)
            .field("cached_level", &self.cached_level)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== ReasoningConfig Tests ==========

    #[test]
    fn test_reasoning_config_clamping() {
        let config = ReasoningConfig::new(3.0, 2.0, 50, 5.0, -5.0);
        assert_eq!(config.temperature, 2.0); // clamped to max
        assert_eq!(config.top_p, 1.0); // clamped to max
        assert_eq!(config.frequency_penalty, 2.0); // clamped to max
        assert_eq!(config.presence_penalty, -2.0); // clamped to min
    }

    #[test]
    fn test_reasoning_config_conservative() {
        let config = ReasoningConfig::conservative();
        assert_eq!(config.temperature, 0.3);
        assert_eq!(config.top_p, 0.5);
        assert_eq!(config.top_k, 10);
    }

    #[test]
    fn test_reasoning_config_maximum_freedom() {
        let config = ReasoningConfig::maximum_freedom();
        assert_eq!(config.temperature, 1.0);
        assert_eq!(config.top_p, 1.0);
        assert_eq!(config.top_k, 100);
    }

    // ========== LinearMapping Tests ==========

    #[test]
    fn test_linear_mapping_boundaries() {
        let strategy = LinearMapping;

        // trust = 0.0 → minimum parameters
        let config_min = strategy.map(0.0, SovereigntyLevel::Level0);
        assert!((config_min.temperature - 0.3).abs() < 0.001);
        assert!((config_min.top_p - 0.5).abs() < 0.001);
        assert_eq!(config_min.top_k, 10);

        // trust = 1.0 → maximum parameters
        let config_max = strategy.map(1.0, SovereigntyLevel::Level3);
        assert!((config_max.temperature - 1.0).abs() < 0.001);
        assert!((config_max.top_p - 1.0).abs() < 0.001);
        assert_eq!(config_max.top_k, 100);
    }

    #[test]
    fn test_linear_mapping_midpoint() {
        let strategy = LinearMapping;
        let config = strategy.map(0.5, SovereigntyLevel::Level1);

        // temperature: 0.3 + 0.5 * 0.7 = 0.65
        assert!((config.temperature - 0.65).abs() < 0.001);
        // top_p: 0.5 + 0.5 * 0.5 = 0.75
        assert!((config.top_p - 0.75).abs() < 0.001);
    }

    // ========== StepMapping Tests ==========

    #[test]
    fn test_step_mapping_levels() {
        let strategy = StepMapping;

        let config_l0 = strategy.map(0.5, SovereigntyLevel::Level0);
        assert_eq!(config_l0.temperature, 0.3);
        assert_eq!(config_l0.top_k, 10);

        let config_l1 = strategy.map(0.65, SovereigntyLevel::Level1);
        assert_eq!(config_l1.temperature, 0.5);
        assert_eq!(config_l1.top_k, 30);

        let config_l2 = strategy.map(0.8, SovereigntyLevel::Level2);
        assert_eq!(config_l2.temperature, 0.7);
        assert_eq!(config_l2.top_k, 60);

        let config_l3 = strategy.map(0.95, SovereigntyLevel::Level3);
        assert_eq!(config_l3.temperature, 0.9);
        assert_eq!(config_l3.top_k, 100);
    }

    // ========== ConservativeMapping Tests ==========

    #[test]
    fn test_conservative_mapping_ceiling() {
        let strategy = ConservativeMapping;
        let config_max = strategy.map(1.0, SovereigntyLevel::Level3);

        // Even at max trust, temperature doesn't exceed 0.7
        assert!((config_max.temperature - 0.7).abs() < 0.001);
        assert!((config_max.top_p - 0.8).abs() < 0.001);
        assert_eq!(config_max.top_k, 50);
    }

    // ========== ReasoningMapper Tests ==========

    #[test]
    fn test_reasoning_mapper_creation() {
        let mapper = ReasoningMapper::new(Box::new(LinearMapping));
        assert_eq!(mapper.strategy_name(), "LinearMapping");
        assert_eq!(mapper.cached_score, 0.0);
    }

    #[test]
    fn test_reasoning_mapper_caching() {
        let mut mapper = ReasoningMapper::new(Box::new(LinearMapping));

        // First computation
        let config1 = mapper.compute(0.5, SovereigntyLevel::Level1);
        let temp1 = config1.temperature;

        // Second computation with same params → should return cached
        let config2 = mapper.compute(0.5, SovereigntyLevel::Level1);
        let temp2 = config2.temperature;

        assert_eq!(temp1, temp2);
        assert_eq!(mapper.cached_score, 0.5);
    }

    #[test]
    fn test_reasoning_mapper_invalidation_on_score_change() {
        let mut mapper = ReasoningMapper::new(Box::new(LinearMapping));

        let config1 = mapper.compute(0.3, SovereigntyLevel::Level0);
        let temp1 = config1.temperature;

        let config2 = mapper.compute(0.7, SovereigntyLevel::Level2);
        let temp2 = config2.temperature;

        // Different scores → different temperatures
        assert!((temp1 - temp2).abs() > 0.001);
    }

    #[test]
    fn test_reasoning_mapper_strategy_swap() {
        let mut mapper = ReasoningMapper::new(Box::new(LinearMapping));
        assert_eq!(mapper.strategy_name(), "LinearMapping");

        mapper.set_strategy(Box::new(ConservativeMapping));
        assert_eq!(mapper.strategy_name(), "ConservativeMapping");

        // Cache invalidated, should recompute with new strategy
        let config = mapper.compute(1.0, SovereigntyLevel::Level3);
        assert!((config.temperature - 0.7).abs() < 0.001); // Conservative max
    }

    #[test]
    fn test_reasoning_mapper_current_config() {
        let mut mapper = ReasoningMapper::new(Box::new(StepMapping));
        mapper.compute(0.65, SovereigntyLevel::Level1);

        let config = mapper.current_config();
        assert_eq!(config.temperature, 0.5); // StepMapping Level 1
    }
}
