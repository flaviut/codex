use super::session::Session;
use super::session::SessionConfiguration;
use super::session::SessionSettingsUpdate;
use crate::config::ConstraintError;
use crate::config::ConstraintResult;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::resolve_model_provider;
use codex_models_manager::manager::SharedModelsManager;
use std::sync::Arc;

impl Session {
    pub(super) fn resolve_provider_update(
        &self,
        current: &SessionConfiguration,
        updates: &SessionSettingsUpdate,
    ) -> ConstraintResult<(SessionConfiguration, SessionSettingsUpdate)> {
        let mut next = current.clone();
        let mut updates = updates.clone();
        let selector = updates
            .step_settings
            .collaboration_mode
            .as_ref()
            .map(|mode| mode.model())
            .or(updates.step_settings.model.as_deref());
        let Some(selector) = selector else {
            return Ok((next, updates));
        };
        let config = &current.original_config_do_not_use;
        let (provider_id, model) = resolve_model_provider(
            selector,
            &config.model_provider_id,
            config.config_layer_stack.required_model_provider(),
            &config.model_providers,
        )
        .map_err(|reason| ConstraintError::ModelProviderSelection { reason })?;
        // Requests already using an explicitly selected provider keep that routing
        // until the model selector changes; sparse effort edits also carry the model.
        let provider_id = if selector == current.step_settings.collaboration_mode.model() {
            config.model_provider_id.clone()
        } else {
            provider_id
        };
        if let Some(mode) = updates.step_settings.collaboration_mode.as_mut() {
            mode.settings.model = model.clone();
        }
        if updates.step_settings.model.is_some() {
            updates.step_settings.model = Some(model);
        }
        if provider_id != config.model_provider_id {
            let mut config = (**config).clone();
            config.model_provider_id = provider_id;
            config.model_provider = config.model_providers[&config.model_provider_id].clone();
            next.provider = create_model_provider(
                config.model_provider.clone(),
                Some(Arc::clone(&self.services.auth_manager)),
            );
            next.original_config_do_not_use = Arc::new(config);
        }
        Ok((next, updates))
    }

    pub(super) fn models_manager_for_configuration(
        &self,
        configuration: &SessionConfiguration,
    ) -> SharedModelsManager {
        if configuration.provider.info() == self.services.model_client.provider_info() {
            Arc::clone(&self.services.models_manager)
        } else {
            self.services
                .model_client
                .for_provider(configuration.provider.clone())
                .provider_models_manager(
                    configuration
                        .original_config_do_not_use
                        .model_catalog
                        .clone(),
                )
        }
    }
}
