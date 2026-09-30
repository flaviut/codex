use super::*;
use crate::built_in_model_providers;
use pretty_assertions::assert_eq;

fn providers() -> HashMap<String, ModelProviderInfo> {
    let mut providers = built_in_model_providers(/*openai_base_url*/ None);
    providers.insert("openrouter".into(), providers["openai"].clone());
    providers.insert("custom".into(), providers["openai"].clone());
    providers
}

#[test]
fn infers_openrouter_and_switches_back_to_openai() {
    for (model, default, expected) in [
        ("z-ai/glm-5.3", "openai", "openrouter"),
        ("openai/gpt-5.5", "openai", "openrouter"),
        ("gpt-5.5", "openrouter", "openai"),
        ("o3", "openrouter", "openai"),
        ("local-model", "custom", "custom"),
        ("gpt-5.5", "custom", "custom"),
        (
            "arn:aws:bedrock:us-east-1::foundation-model/vendor.model",
            "amazon-bedrock",
            "amazon-bedrock",
        ),
    ] {
        assert_eq!(
            resolve_model_provider(
                model,
                default,
                /*explicit_provider*/ None,
                &providers()
            ),
            Ok((expected.into(), model.into()))
        );
    }
}

#[test]
fn explicit_provider_and_qualified_selectors_override_inference() {
    assert_eq!(
        resolve_model_provider("z-ai/glm-5.3", "openai", Some("custom"), &providers()),
        Ok(("custom".into(), "z-ai/glm-5.3".into()))
    );
    assert_eq!(
        resolve_model_provider(
            "custom::z-ai/glm-5.3",
            "openai",
            /*explicit_provider*/ None,
            &providers()
        ),
        Ok(("custom".into(), "z-ai/glm-5.3".into()))
    );
    assert_eq!(
        resolve_model_provider(
            "openrouter::gpt-5.5",
            "openai",
            Some("openrouter"),
            &providers()
        ),
        Ok(("openrouter".into(), "gpt-5.5".into()))
    );
}

#[test]
fn rejects_invalid_or_conflicting_selectors() {
    for selector in ["", "::model", "openrouter::", "missing::model"] {
        assert!(
            resolve_model_provider(
                selector,
                "openai",
                /*explicit_provider*/ None,
                &providers()
            )
            .is_err()
        );
    }
    assert!(
        resolve_model_provider("openrouter::model", "openai", Some("openai"), &providers())
            .is_err()
    );
}

#[test]
fn does_not_infer_an_unconfigured_provider() {
    assert_eq!(
        resolve_model_provider(
            "org/local-model",
            "ollama",
            /*explicit_provider*/ None,
            &built_in_model_providers(/*openai_base_url*/ None)
        ),
        Ok(("ollama".into(), "org/local-model".into()))
    );
}
