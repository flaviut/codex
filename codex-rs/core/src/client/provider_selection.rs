use super::ModelClient;
use codex_model_provider::SharedModelProvider;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelsResponse;
use std::sync::Arc;

impl ModelClient {
    /// Pins provider-specific transport state while retaining this thread's session context.
    /// Cached connections and HTTP fallback decisions are isolated between providers.
    pub(crate) fn for_provider(&self, provider: SharedModelProvider) -> Self {
        if self.state.provider.info() == provider.info() {
            return self.clone();
        }
        let mut states = self
            .provider_states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(state) = states
            .iter()
            .find(|state| state.provider.info() == provider.info())
        {
            return Self {
                state: Arc::clone(state),
                ..self.clone()
            };
        }
        let mut client = Self::new(
            provider.auth_manager(),
            self.agent_identity_policy.clone(),
            self.state.thread_id,
            provider.info().clone(),
            self.state.session_source.clone(),
            self.state.originator.clone(),
            self.state.model_verbosity,
            self.state.content_item_kinds_enabled,
            self.state.reasoning_effort_override_enabled,
            self.state.enable_request_compression,
            self.state.include_timing_metrics,
            self.state.beta_features_header.clone(),
            self.state.concurrent_reasoning_summaries_enabled,
            self.state.attestation_provider.clone(),
            self.http_client_factory.clone(),
            self.state.workspace_routing.clone(),
            self.request_contributors.clone(),
        );
        Arc::get_mut(&mut client.state)
            .expect("new client owns its state")
            .provider = provider;
        states.push(Arc::clone(&client.state));
        Self {
            state: client.state,
            ..self.clone()
        }
    }

    pub(crate) fn provider_models_manager(
        &self,
        catalog: Option<ModelsResponse>,
    ) -> SharedModelsManager {
        Arc::clone(
            self.state
                .models_manager
                .get_or_init(|| self.state.provider.models_manager_without_cache(catalog)),
        )
    }
}
