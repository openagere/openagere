pub(crate) struct SessionHeader {
    provider: String,
    model: String,
}

impl SessionHeader {
    pub(crate) fn new(provider: String, model: String) -> Self {
        Self { provider, model }
    }

    /// Updates the header's model text.
    pub(crate) fn set_model(&mut self, model: &str) {
        if self.model != model {
            self.model = model.to_string();
        }
    }

    /// Updates the header's provider text.
    pub(crate) fn set_provider(&mut self, provider: &str) {
        if self.provider != provider {
            self.provider = provider.to_string();
        }
    }

    pub(crate) fn provider(&self) -> &str {
        &self.provider
    }
}
