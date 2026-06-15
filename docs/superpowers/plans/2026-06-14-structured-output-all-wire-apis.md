# Structured Output All Wire APIs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `turn/start.outputSchema` work for Responses, OpenAI Chat Completions, and Anthropic Messages without silent schema drops after provider switches.

**Architecture:** Keep app-server v2 API unchanged. Responses keeps `text.format`; Chat adds `response_format`; Anthropic extends `output_config.format` while preserving reasoning effort.

**Tech Stack:** Rust, serde, `agere-core`, `openai-chat-client`, `anthropic-client`, app-server v2 tests.

---

## File Structure

- `openai-chat-client/src/types.rs`: add `ChatRequest.response_format`, `ChatResponseFormat`, `ChatJsonSchemaFormat`.
- `openai-chat-client/src/lib.rs`: add `ChatOptions.output_schema` and `output_schema_strict`.
- `openai-chat-client/src/client.rs`: build `response_format` from `ChatOptions`.
- `anthropic-client/src/types.rs`: add `OutputConfig.format` and `OutputFormat::JsonSchema`.
- `anthropic-client/src/lib.rs`: add `AnthropicOptions.output_schema`.
- `anthropic-client/src/client.rs`: merge schema format into `output_config`.
- `core/src/client.rs`: pass `Prompt.output_schema` to Chat and Anthropic options.
- `app-server/tests/suite/v2/output_schema.rs`: test provider-switched Chat and Anthropic schema propagation.
- `app-server/README.md`: document native mappings.

## Task 1: OpenAI Chat Mapping

- [ ] Add failing serde test in `openai-chat-client/src/types.rs` for `response_format: { type: "json_schema", json_schema: { name: "agere_output_schema", schema, strict: true } }`.
- [ ] Run `cargo test -p agere-openai-chat-client serialize_response_format_json_schema`; expect compile failure.
- [ ] Add `ChatResponseFormat` and `ChatJsonSchemaFormat`; add optional `response_format` to `ChatRequest`; set `response_format: None` in existing request literals.
- [ ] Add `output_schema: Option<serde_json::Value>` and `output_schema_strict: bool` to `ChatOptions`; default to `None` and `true`.
- [ ] In `openai-chat-client/src/client.rs`, map `options.output_schema` to `ChatRequest.response_format` with name `agere_output_schema`.
- [ ] Run `cargo test -p agere-openai-chat-client`; expect PASS.

## Task 2: Anthropic Mapping

- [ ] Add failing serde test in `anthropic-client/src/types.rs` for `output_config: { effort: "low", format: { type: "json_schema", schema } }`.
- [ ] Run `cargo test -p agere-anthropic-client serialize_output_config_json_schema_format`; expect compile failure.
- [ ] Add `OutputFormat::JsonSchema { schema: serde_json::Value }`; add optional `format` to `OutputConfig`; update existing `OutputConfig` literals with `format: None`.
- [ ] Add `output_schema: Option<serde_json::Value>` to `AnthropicOptions`; default existing construction sites to `None`.
- [ ] In `anthropic-client/src/client.rs`, add a private merge helper that inserts schema into existing `output_config` or creates `OutputConfig { effort: None, format: Some(...) }`.
- [ ] Use the helper in both `stream_request_with_messages` and `stream_request`.
- [ ] Run `cargo test -p agere-anthropic-client`; expect PASS.

## Task 3: Core Wiring

- [ ] In `core/src/client.rs:1658`, make Anthropic options mutable and set `options.output_schema = prompt.output_schema.clone()`.
- [ ] In `core/src/client.rs:1742`, make Chat options mutable and set `options.output_schema = prompt.output_schema.clone(); options.output_schema_strict = prompt.output_schema_strict`.
- [ ] Run `cargo test -p agere-core client_common`; expect PASS or no matching tests after compile.

## Task 4: App-Server Tests

- [ ] Inspect helpers with `rg "provider|wire_api|turn_start|output_schema|body_json|single_request" app-server/tests/suite/v2 app-server/tests/common -n`.
- [ ] Add Chat provider-switch test: Responses thread -> `/provider` Chat -> `turn/start` with `outputSchema` -> outbound body contains `response_format.json_schema.schema`.
- [ ] Add Anthropic provider-switch test: Responses thread -> `/provider` Anthropic -> `turn/start` with `outputSchema` -> outbound body contains `output_config.format.schema`.
- [ ] Add no-schema regression assertions: Chat omits `response_format`; Anthropic omits `output_config.format`.
- [ ] Run `cargo test -p agere-app-server output_schema`; expect PASS.

## Task 5: Docs And Validation

- [ ] Update `app-server/README.md:613` to document Responses `text.format`, Chat `response_format`, and Anthropic `output_config.format` mappings.
- [ ] Run `just fmt`; expect PASS.
- [ ] Run `just fix -p agere-openai-chat-client`, `just fix -p agere-anthropic-client`, `just fix -p agere-core`, and `just fix -p agere-app-server`; expect PASS.
- [ ] Run changed project tests if not already passed.
- [ ] Ask before full `cargo test` because `core` changed.

## Self-Review

- Covers native schema mapping for Chat and Anthropic while leaving Responses unchanged.
- Prevents silent provider-switch schema drops with outbound request-body tests.
- Avoids model allowlists and schema rewriting; provider errors remain authoritative.
