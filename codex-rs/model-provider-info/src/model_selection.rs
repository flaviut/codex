use crate::ModelProviderInfo;
use crate::OPENAI_PROVIDER_ID;
use std::collections::HashMap;

/// Resolves a model selector to a configured provider and its unqualified model ID.
/// Explicit provider choices take precedence over inference. `provider::model`
/// disambiguates models shared by several providers without changing their API ID.
pub fn resolve_model_provider(
    selector: &str,
    default_provider: &str,
    explicit_provider: Option<&str>,
    providers: &HashMap<String, ModelProviderInfo>,
) -> Result<(String, String), String> {
    let (qualified_provider, model) = match selector
        .split_once("::")
        .filter(|(provider, _)| !provider.contains(':'))
    {
        Some((provider, model)) => (Some(provider), model),
        None => (None, selector),
    };
    if model.is_empty() || qualified_provider == Some("") {
        return Err("Expected a model name or provider::model".to_string());
    }
    if let (Some(explicit), Some(qualified)) = (explicit_provider, qualified_provider)
        && explicit != qualified
    {
        return Err(format!(
            "Model selector requests provider `{qualified}`, but provider `{explicit}` is required"
        ));
    }
    let provider = explicit_provider.or(qualified_provider).unwrap_or_else(|| {
        if model.split_once('/').is_some_and(|(namespace, model)| {
            !namespace.is_empty() && !namespace.contains(':') && !model.is_empty()
        }) && providers.contains_key("openrouter")
        {
            "openrouter"
        } else if default_provider == "openrouter"
            && (model.starts_with("gpt-")
                || model.starts_with("chatgpt-")
                || matches!(model.split('-').next(), Some("o1" | "o3" | "o4")))
        {
            OPENAI_PROVIDER_ID
        } else {
            default_provider
        }
    });
    if !providers.contains_key(provider) {
        if provider == crate::LEGACY_OLLAMA_CHAT_PROVIDER_ID {
            return Err(crate::OLLAMA_CHAT_PROVIDER_REMOVED_ERROR.to_string());
        }
        return Err(format!("Model provider `{provider}` not found"));
    }
    Ok((provider.to_string(), model.to_string()))
}

#[cfg(test)]
#[path = "model_selection_tests.rs"]
mod tests;
