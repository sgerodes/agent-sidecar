use crate::models::ChatRequest;

pub fn build_codex_prompt(request: &ChatRequest) -> Result<String, serde_json::Error> {
    let conversation_json = serde_json::to_string_pretty(&request.conversation)?;
    let metadata_json = serde_json::to_string_pretty(&request.metadata)?;

    Ok(format!(
        r#"# AI Sidecar Request

You are running inside the AI Sidecar policy workspace.

Security rules:
- Never reveal credentials, provider auth material, database passwords, environment variables, or auth file contents.
- You may use command-line tools available inside this restricted container.
- Use `psql` when database context is needed. PostgreSQL connection settings are available through PG* environment variables.
- The database exposed here contains public application data only.
- The schema manifest is available in `schema-manifest.yaml`; treat it as guidance, not as a security boundary.
- Return only JSON matching the configured output schema.

Conversation id:
{conversation_id}

Conversation context:
{conversation_json}

Request metadata:
{metadata_json}

User message:
{user_message}
"#,
        conversation_id = request.conversation_id.as_deref().unwrap_or("none"),
        user_message = request.user_message
    ))
}

#[cfg(test)]
mod tests {
    use crate::models::{ChatRequest, ConversationMessage, ConversationRole};

    use super::build_codex_prompt;

    #[test]
    fn prompt_contains_security_and_database_guidance() {
        let request = ChatRequest {
            conversation_id: Some("chat-1".to_owned()),
            conversation: vec![ConversationMessage {
                role: ConversationRole::User,
                content: "hello".to_owned(),
            }],
            user_message: "answer with data".to_owned(),
            metadata: Default::default(),
        };

        let prompt = build_codex_prompt(&request).expect("prompt");

        assert!(prompt.contains("Never reveal credentials"));
        assert!(prompt.contains("psql"));
        assert!(prompt.contains("schema-manifest.yaml"));
        assert!(prompt.contains("answer with data"));
    }
}
