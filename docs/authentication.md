# Authentication

OpenAgere supports multiple authentication methods for connecting to AI model providers.

## Login flow

Run the login command to authenticate:

```shell
openagere login
```

This opens a browser window for OAuth-based authentication. After completing the flow, your credentials are stored securely.

## Supported providers

OpenAgere works with:

- **OpenAI** — API key or OAuth
- **Anthropic** — API key
- **OpenRouter** — API key
- **Custom providers** — Any OpenAI-compatible API endpoint

## Configuring API keys

You can set API keys directly in `~/.openagere/config.toml`:

```toml
[model_providers.openai]
api_key = "sk-..."

[model_providers.anthropic]
api_key = "sk-ant-..."
```

Or via environment variables:

```shell
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

## Custom provider endpoints

```toml
[model_providers.custom]
base_url = "https://api.example.com/v1"
api_key = "your-key"
models = ["model-name-1", "model-name-2"]
```

## Logout

```shell
openagere logout
```

## Credential storage

Credentials are stored:

- **macOS:** Keychain
- **Linux:** Secret Service API (via keyring-store crate)
- **Windows:** Credential Manager

The `agere-secrets` crate handles secure storage and redaction of sensitive values in logs and traces.

## Check authentication status

```shell
openagere login --status
```
