use super::*;
use codex_config::profile_toml::ConfigProfile;
use codex_model_provider_info::resolve_model_provider;
use std::collections::BTreeMap;

impl ChatWidget {
    pub(super) fn with_profile_models(&self, mut presets: Vec<ModelPreset>) -> Vec<ModelPreset> {
        let config = self.config.config_layer_stack.effective_config();
        let profiles = config
            .get("profiles")
            .cloned()
            .and_then(|profiles| profiles.try_into::<BTreeMap<String, ConfigProfile>>().ok())
            .unwrap_or_default();
        for (name, profile) in profiles {
            let Some(model) = profile.model else {
                continue;
            };
            let Ok((provider, model)) = resolve_model_provider(
                &model,
                &self.config.model_provider_id,
                profile.model_provider.as_deref(),
                &self.config.model_providers,
            ) else {
                continue;
            };
            let inferred = resolve_model_provider(
                &model,
                &self.config.model_provider_id,
                /*explicit_provider*/ None,
                &self.config.model_providers,
            );
            let selector = if inferred.as_ref().is_ok_and(|(id, _)| id == &provider) {
                model.clone()
            } else {
                format!("{provider}::{model}")
            };
            if presets.iter().any(|preset| preset.model == selector) {
                continue;
            }
            let effort = profile
                .model_reasoning_effort
                .unwrap_or(ReasoningEffortConfig::None);
            presets.push(ModelPreset {
                id: selector.clone(),
                model: selector,
                display_name: model,
                description: format!("{provider} · profile {name}"),
                model_specialty: None,
                default_reasoning_effort: effort,
                supported_reasoning_efforts: Vec::new(),
                supports_personality: false,
                additional_speed_tiers: Vec::new(),
                service_tiers: Vec::new(),
                default_service_tier: None,
                available_access_programs: None,
                is_default: false,
                upgrade: None,
                show_in_picker: true,
                multi_agent_version: None,
                availability_nux: None,
                supported_in_api: true,
                input_modalities: vec![codex_protocol::openai_models::InputModality::Text],
            });
        }
        if let Some(pattern) = self.config.tui_model_picker_filter.as_deref() {
            if let Ok(filter) = regex_lite::Regex::new(pattern) {
                presets.retain(|preset| filter.is_match(&preset.model));
            }
        }
        presets
    }
}
