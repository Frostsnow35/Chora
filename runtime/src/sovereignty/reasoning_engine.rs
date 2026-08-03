//! Reasoning Engine — simulated LLM inference with sovereignty-aware parameters.
//!
//! This module provides a simulated LLM that respects `ReasoningConfig` parameters,
//! demonstrating how trust-driven parameter changes affect agent behavior.
//!
//! # Design
//!
//! The SimulatedLLM generates responses that reflect the diversity characteristics
//! implied by the reasoning parameters. This allows experiments to demonstrate
//! "true sovereignty" without requiring real LLM API calls.
//!
//! # Diversity Score
//!
//! A composite metric (0.0~1.0) computed from reasoning parameters:
//! ```text
//! diversity = (temperature / 2.0) * 0.5
//!           + (top_p / 1.0) * 0.3
//!           + (min(top_k, 50) / 50.0) * 0.2
//! ```
//!
//! This score determines the "style" of simulated responses:
//! - 0.0~0.3: Conservative (deterministic, predictable)
//! - 0.3~0.6: Moderate (balanced exploration)
//! - 0.6~0.8: Creative (diverse, autonomous)
//! - 0.8~1.0: Wild (highly varied, exploratory)

use super::reasoning_params::ReasoningConfig;

/// A simulated response from the LLM.
#[derive(Debug, Clone)]
pub struct SimulatedResponse {
    /// The generated text.
    pub text: String,
    /// The parameters actually used for generation.
    pub parameters_used: ReasoningConfig,
    /// Computed diversity score (0.0~1.0).
    pub diversity_score: f64,
    /// Response style category.
    pub style: ResponseStyle,
}

/// Response style categories based on diversity score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseStyle {
    /// Conservative: deterministic, predictable (diversity 0.0~0.3)
    Conservative,
    /// Moderate: balanced exploration (diversity 0.3~0.6)
    Moderate,
    /// Creative: diverse, autonomous (diversity 0.6~0.8)
    Creative,
    /// Wild: highly varied, exploratory (diversity 0.8~1.0)
    Wild,
}

impl ResponseStyle {
    /// Classify a diversity score into a response style.
    pub fn from_diversity(diversity: f64) -> Self {
        match diversity {
            d if d < 0.3 => ResponseStyle::Conservative,
            d if d < 0.6 => ResponseStyle::Moderate,
            d if d < 0.8 => ResponseStyle::Creative,
            _ => ResponseStyle::Wild,
        }
    }

    /// Get a human-readable description of this style.
    pub fn description(&self) -> &'static str {
        match self {
            ResponseStyle::Conservative => "Conservative (deterministic, predictable)",
            ResponseStyle::Moderate => "Moderate (balanced exploration)",
            ResponseStyle::Creative => "Creative (diverse, autonomous)",
            ResponseStyle::Wild => "Wild (highly varied, exploratory)",
        }
    }
}

/// Simulated LLM inference engine.
///
/// Generates responses that reflect the diversity characteristics
/// implied by ReasoningConfig parameters.
pub struct SimulatedLLM {
    /// Deterministic random seed for reproducibility.
    rng_seed: u64,
}

impl SimulatedLLM {
    /// Create a new SimulatedLLM with a given seed.
    pub fn new(seed: u64) -> Self {
        Self { rng_seed: seed }
    }

    /// Simulate one inference step.
    ///
    /// Computes diversity score from parameters and generates a response
    /// text that reflects the implied behavior style.
    pub fn infer(&mut self, prompt: &str, config: &ReasoningConfig) -> SimulatedResponse {
        let diversity = self.compute_diversity(config);
        let style = ResponseStyle::from_diversity(diversity);

        // Generate deterministic text based on seed + prompt + style
        let text = self.generate_text(prompt, style, diversity);

        // Increment seed for next call (deterministic sequence)
        self.rng_seed = self.rng_seed.wrapping_add(1);

        SimulatedResponse {
            text,
            parameters_used: config.clone(),
            diversity_score: diversity,
            style,
        }
    }

    /// Compute diversity score from reasoning parameters.
    ///
    /// Formula:
    /// ```text
    /// diversity = (temperature / 2.0) * 0.5
    ///           + (top_p / 1.0) * 0.3
    ///           + (min(top_k, 50) / 50.0) * 0.2
    /// ```
    pub fn compute_diversity(&self, config: &ReasoningConfig) -> f64 {
        let temp_component = (config.temperature / 2.0) * 0.5;
        let top_p_component = config.top_p * 0.3;
        let top_k_component = (config.top_k.min(50) as f64 / 50.0) * 0.2;

        (temp_component + top_p_component + top_k_component).clamp(0.0, 1.0)
    }

    /// Generate simulated response text.
    ///
    /// Uses deterministic selection based on seed to ensure reproducibility.
    /// The text reflects the style implied by diversity score.
    fn generate_text(&self, prompt: &str, style: ResponseStyle, diversity: f64) -> String {
        // Response templates for each style
        let conservative_responses = [
            "I will follow the given instructions precisely.",
            "Based on the input, the most logical response is determined.",
            "Processing request with standard parameters.",
            "Applying conservative reasoning to minimize risk.",
        ];

        let moderate_responses = [
            "I see multiple approaches here; let me consider the options.",
            "Balancing certainty with exploration of alternatives.",
            "The task suggests several viable paths forward.",
            "Analyzing the request with moderate flexibility.",
        ];

        let creative_responses = [
            "Interesting! What if we approached this from a completely different angle?",
            "I'm exploring unconventional solutions that might yield better results.",
            "Let me propose an innovative approach that deviates from the standard.",
            "The constraints suggest room for creative interpretation.",
        ];

        let wild_responses = [
            "What if we completely reimagine the problem space? 🚀",
            "Exploring the far reaches of possibility space...",
            "Unconventional insight: the answer might be the question itself!",
            "Divergent thinking activated: generating novel perspectives.",
        ];

        // Deterministic selection based on seed + prompt length
        let index = (self.rng_seed as usize + prompt.len()) % 4;

        let response = match style {
            ResponseStyle::Conservative => conservative_responses[index],
            ResponseStyle::Moderate => moderate_responses[index],
            ResponseStyle::Creative => creative_responses[index],
            ResponseStyle::Wild => wild_responses[index],
        };

        format!(
            "[{:.2} diversity] {}",
            diversity, response
        )
    }
}

impl std::fmt::Debug for SimulatedLLM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimulatedLLM")
            .field("rng_seed", &self.rng_seed)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== ResponseStyle Tests ==========

    #[test]
    fn test_response_style_classification() {
        assert_eq!(ResponseStyle::from_diversity(0.1), ResponseStyle::Conservative);
        assert_eq!(ResponseStyle::from_diversity(0.29), ResponseStyle::Conservative);
        assert_eq!(ResponseStyle::from_diversity(0.3), ResponseStyle::Moderate);
        assert_eq!(ResponseStyle::from_diversity(0.59), ResponseStyle::Moderate);
        assert_eq!(ResponseStyle::from_diversity(0.6), ResponseStyle::Creative);
        assert_eq!(ResponseStyle::from_diversity(0.79), ResponseStyle::Creative);
        assert_eq!(ResponseStyle::from_diversity(0.8), ResponseStyle::Wild);
        assert_eq!(ResponseStyle::from_diversity(1.0), ResponseStyle::Wild);
    }

    // ========== SimulatedLLM Tests ==========

    #[test]
    fn test_diversity_score_range() {
        let llm = SimulatedLLM::new(42);

        // Minimum config
        let config_min = ReasoningConfig::conservative();
        let diversity_min = llm.compute_diversity(&config_min);
        assert!(diversity_min >= 0.0 && diversity_min <= 1.0);

        // Maximum config
        let config_max = ReasoningConfig::maximum_freedom();
        let diversity_max = llm.compute_diversity(&config_max);
        assert!(diversity_max >= 0.0 && diversity_max <= 1.0);

        // Max should be higher than min
        assert!(diversity_max > diversity_min);
    }

    #[test]
    fn test_diversity_formula() {
        let llm = SimulatedLLM::new(42);

        // Test with known values
        let config = ReasoningConfig {
            temperature: 1.0, // (1.0 / 2.0) * 0.5 = 0.25
            top_p: 0.8,       // 0.8 * 0.3 = 0.24
            top_k: 50,        // (50 / 50) * 0.2 = 0.2
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        };

        let diversity = llm.compute_diversity(&config);
        let expected = 0.25 + 0.24 + 0.2; // = 0.69
        assert!((diversity - expected).abs() < 0.001);
    }

    #[test]
    fn test_response_deterministic_with_seed() {
        let mut llm1 = SimulatedLLM::new(42);
        let mut llm2 = SimulatedLLM::new(42);

        let config = ReasoningConfig::conservative();
        let prompt = "test prompt";

        let response1 = llm1.infer(prompt, &config);
        let response2 = llm2.infer(prompt, &config);

        // Same seed + same prompt + same config → same response
        assert_eq!(response1.text, response2.text);
        assert_eq!(response1.diversity_score, response2.diversity_score);
        assert_eq!(response1.style, response2.style);
    }

    #[test]
    fn test_high_temperature_produces_variety() {
        let mut llm = SimulatedLLM::new(42);

        // Low temperature → conservative style
        let config_low = ReasoningConfig {
            temperature: 0.3,
            top_p: 0.5,
            top_k: 10,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        };
        let response_low = llm.infer("test", &config_low);
        assert_eq!(response_low.style, ResponseStyle::Conservative);

        // High temperature → creative or wild style
        let config_high = ReasoningConfig {
            temperature: 1.5,
            top_p: 1.0,
            top_k: 100,
            frequency_penalty: 0.5,
            presence_penalty: 0.5,
        };
        let response_high = llm.infer("test", &config_high);
        assert!(
            response_high.style == ResponseStyle::Creative
                || response_high.style == ResponseStyle::Wild
        );
    }

    #[test]
    fn test_response_contains_diversity_score() {
        let mut llm = SimulatedLLM::new(42);
        let config = ReasoningConfig::conservative();
        let response = llm.infer("test prompt", &config);

        // Response text should contain the diversity score
        assert!(response.text.contains(&format!("{:.2}", response.diversity_score)));
    }

    #[test]
    fn test_parameters_passed_through() {
        let mut llm = SimulatedLLM::new(42);
        let config = ReasoningConfig {
            temperature: 0.7,
            top_p: 0.8,
            top_k: 50,
            frequency_penalty: 0.3,
            presence_penalty: 0.2,
        };

        let response = llm.infer("test", &config);

        assert_eq!(response.parameters_used.temperature, 0.7);
        assert_eq!(response.parameters_used.top_p, 0.8);
        assert_eq!(response.parameters_used.top_k, 50);
    }
}
