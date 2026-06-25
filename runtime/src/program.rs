//! AgentProgram — the three-layer description of what an agent is.

use serde::{Deserialize, Serialize};

/// The reasoning substrate (model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// Provider (e.g., "openai", "anthropic").
    pub provider: String,
    /// Model name (e.g., "gpt-4o", "claude-3-opus").
    pub model_name: String,
    /// Configuration as JSON value.
    pub config: serde_json::Value,
}

impl ModelDescriptor {
    pub fn new(provider: impl Into<String>, model_name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_name: model_name.into(),
            config: serde_json::json!({}),
        }
    }
}

/// The instructions that shape behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDescriptor {
    /// System prompt defining persona and role.
    pub system_prompt: String,
    /// Optional persona summary.
    pub persona: Option<String>,
    /// Behavioral rules and constraints.
    pub rules: Vec<String>,
}

impl PromptDescriptor {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            persona: None,
            rules: vec![],
        }
    }

    pub fn with_persona(mut self, persona: impl Into<String>) -> Self {
        self.persona = Some(persona.into());
        self
    }

    pub fn with_rule(mut self, rule: impl Into<String>) -> Self {
        self.rules.push(rule.into());
        self
    }
}

/// Tools the agent can invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSet {
    pub tools: Vec<ToolDescriptor>,
}

impl Default for ToolSet {
    fn default() -> Self {
        Self { tools: vec![] }
    }
}

impl ToolSet {
    pub fn new(tools: Vec<ToolDescriptor>) -> Self {
        Self { tools }
    }

    pub fn add_tool(mut self, tool: ToolDescriptor) -> Self {
        self.tools.push(tool);
        self
    }
}

/// Single tool descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    /// Tool name (for invocation).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON schema for arguments.
    pub schema: serde_json::Value,
}

impl ToolDescriptor {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            schema,
        }
    }
}

/// The three-layer program describing an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgram {
    /// Reasoning substrate.
    pub model: ModelDescriptor,
    /// Behavior-shaping instructions.
    pub prompt: PromptDescriptor,
    /// Available tools.
    pub tools: ToolSet,
}

impl AgentProgram {
    pub fn new(model: ModelDescriptor, prompt: PromptDescriptor) -> Self {
        Self {
            model,
            prompt,
            tools: ToolSet::default(),
        }
    }

    pub fn with_tools(mut self, tools: ToolSet) -> Self {
        self.tools = tools;
        self
    }

    pub fn add_tool(&mut self, tool: ToolDescriptor) {
        self.tools.tools.push(tool);
    }
}
