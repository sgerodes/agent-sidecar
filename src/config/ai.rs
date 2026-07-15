use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiProviderKind {
    #[default]
    Codex,
    Claude,
}


#[derive(Debug, Clone, Default, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub ai_provider: AiProviderKind,
}
