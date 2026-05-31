use axga_shared::error::{AxgaError, AxgaResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Debug, Clone, Copy)]
pub struct ProviderSpec {
    pub name: &'static str,
    pub kind: ProviderKind,
    pub api_key_env: Option<&'static str>,
    pub default_base_url: Option<&'static str>,
    pub default_model: &'static str,
    pub models: &'static [&'static str],
    pub requires_api_key: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub spec: &'static ProviderSpec,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

const OPENAI_MODELS: &[&str] = &["gpt-4o-mini", "gpt-4o", "gpt-4.1", "o3-mini"];
const DEEPSEEK_MODELS: &[&str] = &["deepseek-chat", "deepseek-reasoner"];
const ANTHROPIC_MODELS: &[&str] = &["claude-sonnet-4-20250514", "claude-haiku-3-5"];
const OPENROUTER_MODELS: &[&str] = &[
    "openai/gpt-4o-mini",
    "anthropic/claude-3.5-sonnet",
    "deepseek/deepseek-chat",
];
const GROQ_MODELS: &[&str] = &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"];
const OLLAMA_MODELS: &[&str] = &["llama3.2", "qwen2.5-coder", "mistral"];

pub const PROVIDERS: &[ProviderSpec] = &[
    ProviderSpec {
        name: "openai",
        kind: ProviderKind::OpenAiCompatible,
        api_key_env: Some("OPENAI_API_KEY"),
        default_base_url: Some("https://api.openai.com/v1"),
        default_model: "gpt-4o-mini",
        models: OPENAI_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        name: "deepseek",
        kind: ProviderKind::OpenAiCompatible,
        api_key_env: Some("DEEPSEEK_API_KEY"),
        default_base_url: Some("https://api.deepseek.com/v1"),
        default_model: "deepseek-chat",
        models: DEEPSEEK_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        name: "anthropic",
        kind: ProviderKind::Anthropic,
        api_key_env: Some("ANTHROPIC_API_KEY"),
        default_base_url: None,
        default_model: "claude-sonnet-4-20250514",
        models: ANTHROPIC_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        name: "openrouter",
        kind: ProviderKind::OpenAiCompatible,
        api_key_env: Some("OPENROUTER_API_KEY"),
        default_base_url: Some("https://openrouter.ai/api/v1"),
        default_model: "openai/gpt-4o-mini",
        models: OPENROUTER_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        name: "groq",
        kind: ProviderKind::OpenAiCompatible,
        api_key_env: Some("GROQ_API_KEY"),
        default_base_url: Some("https://api.groq.com/openai/v1"),
        default_model: "llama-3.3-70b-versatile",
        models: GROQ_MODELS,
        requires_api_key: true,
    },
    ProviderSpec {
        name: "ollama",
        kind: ProviderKind::OpenAiCompatible,
        api_key_env: None,
        default_base_url: Some("http://localhost:11434/v1"),
        default_model: "llama3.2",
        models: OLLAMA_MODELS,
        requires_api_key: false,
    },
];

pub fn provider_specs() -> &'static [ProviderSpec] {
    PROVIDERS
}

pub fn provider_spec(name: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS
        .iter()
        .find(|provider| provider.name.eq_ignore_ascii_case(name))
}

pub fn default_model_for_provider(provider: &str) -> AxgaResult<&'static str> {
    provider_spec(provider)
        .map(|spec| spec.default_model)
        .ok_or_else(|| unknown_provider(provider))
}

pub fn resolve_provider(
    provider: &str,
    api_key: Option<&str>,
    base_url: Option<&str>,
) -> AxgaResult<ResolvedProvider> {
    let spec = provider_spec(provider).ok_or_else(|| unknown_provider(provider))?;
    let api_key = api_key
        .map(ToOwned::to_owned)
        .or_else(|| spec.api_key_env.and_then(|env| std::env::var(env).ok()));

    if spec.requires_api_key && api_key.is_none() {
        let env = spec.api_key_env.unwrap_or("API key env var");
        return Err(AxgaError::Config(format!(
            "{env} not set for provider '{}'",
            spec.name
        )));
    }

    Ok(ResolvedProvider {
        spec,
        api_key,
        base_url: base_url
            .map(ToOwned::to_owned)
            .or_else(|| spec.default_base_url.map(ToOwned::to_owned)),
    })
}

fn unknown_provider(provider: &str) -> AxgaError {
    let names = PROVIDERS
        .iter()
        .map(|spec| spec.name)
        .collect::<Vec<_>>()
        .join(", ");
    AxgaError::Config(format!("unknown provider: {provider} (supported: {names})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_openai_compatible_defaults() {
        let resolved = resolve_provider("deepseek", Some("test-key"), None).unwrap();

        assert_eq!(resolved.spec.kind, ProviderKind::OpenAiCompatible);
        assert_eq!(resolved.api_key.as_deref(), Some("test-key"));
        assert_eq!(
            resolved.base_url.as_deref(),
            Some("https://api.deepseek.com/v1")
        );
        assert_eq!(
            default_model_for_provider("deepseek").unwrap(),
            "deepseek-chat"
        );
    }

    #[test]
    fn ollama_does_not_require_api_key() {
        let resolved = resolve_provider("ollama", None, None).unwrap();

        assert_eq!(resolved.spec.kind, ProviderKind::OpenAiCompatible);
        assert!(resolved.api_key.is_none());
        assert_eq!(
            resolved.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
    }

    #[test]
    fn explicit_base_url_overrides_provider_default() {
        let resolved = resolve_provider(
            "openrouter",
            Some("test-key"),
            Some("http://proxy.local/v1"),
        )
        .unwrap();

        assert_eq!(resolved.api_key.as_deref(), Some("test-key"));
        assert_eq!(resolved.base_url.as_deref(), Some("http://proxy.local/v1"));
    }

    #[test]
    fn unknown_provider_is_rejected() {
        assert!(resolve_provider("missing", None, None).is_err());
    }
}
