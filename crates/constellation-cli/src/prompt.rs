//! Template rendering for the LLM setup prompt.

const TEMPLATE: &str = include_str!("../../../docs/setup-prompt.md");

/// Render the setup prompt template, substituting agent metadata.
pub fn render(agent_name: &str, skills: &[String], local_url: &str, store_path: &str) -> String {
    TEMPLATE
        .replace("{{AGENT_NAME}}", agent_name)
        .replace("{{AGENT_SKILLS}}", &skills.join(", "))
        .replace("{{LOCAL_URL}}", local_url)
        .replace("{{STORE_PATH}}", store_path)
}
