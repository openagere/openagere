use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use agere_models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;
use agere_protocol::config_types::CollaborationModeMask;
use agere_protocol::openai_models::ModelPreset;
use std::convert::Infallible;

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalog {
    models: Vec<ModelPreset>,
    collaboration_modes_config: CollaborationModesConfig,
}

impl ModelCatalog {
    pub(crate) fn new(
        models: Vec<ModelPreset>,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Self {
        Self {
            models,
            collaboration_modes_config,
        }
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self.models.clone())
    }

    pub(crate) fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        builtin_collaboration_mode_presets(self.collaboration_modes_config)
    }

    /// Look up the context window size for a model by its slug.
    pub(crate) fn find_model_context_window(&self, model: &str) -> Option<i64> {
        self.models
            .iter()
            .find(|m| m.model == model)
            .and_then(|m| m.context_window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn list_collaboration_modes_matches_core_presets() {
        let collaboration_modes_config = CollaborationModesConfig {
            default_mode_request_user_input: true,
        };
        let catalog = ModelCatalog::new(Vec::new(), collaboration_modes_config);

        assert_eq!(
            catalog.list_collaboration_modes(),
            builtin_collaboration_mode_presets(collaboration_modes_config)
        );
    }
}
