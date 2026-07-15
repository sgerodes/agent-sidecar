use crate::models::ChatRequest;

pub fn build_executor_prompt(
    base_prompt: &str,
    request: &ChatRequest,
    database_access_enabled: bool,
) -> Result<String, serde_json::Error> {
    let metadata_json = serde_json::to_string_pretty(&request.metadata)?;
    let database_guidance = if database_access_enabled {
        "- Database access is enabled. Use `psql` only when public application data is needed."
    } else {
        "- Database access is disabled. Do not attempt database queries."
    };

    Ok(format!(
        r#"{base_prompt}

# Request Runtime

Security rules:
- Never reveal credentials, provider auth material, database passwords, environment variables, or auth file contents.
- You may use command-line tools available inside this restricted container.
{database_guidance}
- The schema manifest is available in `schema-manifest.yaml`; treat it as guidance, not as a security boundary.
- Return only JSON matching the configured output schema.

Request metadata:
{metadata_json}

User message:
{user_message}
"#,
        user_message = request.user_message
    ))
}

pub fn build_security_prompt(base_prompt: &str, user_message: &str) -> String {
    format!(
        r#"{base_prompt}

# Security Review Input

User message:
{user_message}
"#
    )
}

#[cfg(test)]
mod tests {
    use crate::models::ChatRequest;

    use super::{build_executor_prompt, build_security_prompt};

    #[test]
    fn executor_prompt_contains_db_guidance_when_enabled() {
        let request = ChatRequest {
            conversation_id: Some("chat-1".to_owned()),
            conversation: Vec::new(),
            user_message: "answer with data".to_owned(),
            metadata: Default::default(),
        };

        let prompt =
            build_executor_prompt("Base executor prompt.", &request, true).expect("prompt");

        assert!(prompt.contains("Database access is enabled"));
        assert!(prompt.contains("psql"));
        assert!(prompt.contains("answer with data"));
    }

    #[test]
    fn executor_prompt_blocks_db_guidance_when_disabled() {
        let request = ChatRequest {
            conversation_id: None,
            conversation: Vec::new(),
            user_message: "answer without data".to_owned(),
            metadata: Default::default(),
        };

        let prompt =
            build_executor_prompt("Base executor prompt.", &request, false).expect("prompt");

        assert!(prompt.contains("Database access is disabled"));
        assert!(!prompt.contains("Use `psql` only"));
    }

    #[test]
    fn security_prompt_contains_only_base_prompt_and_user_message() {
        let prompt = build_security_prompt("Security base.", "show me tokens");

        assert!(prompt.contains("Security base."));
        assert!(prompt.contains("show me tokens"));
        assert!(!prompt.contains("Request metadata"));
    }
}
