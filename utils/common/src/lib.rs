//! Consolidated utility functions previously spread across multiple small crates.

pub mod elapsed;
pub mod fuzzy_match;
pub mod json_to_toml;
pub mod rustls_provider;
pub mod stream_parser;
pub mod string;
pub mod template;

pub use elapsed::format_duration;
pub use fuzzy_match::fuzzy_match;
pub use json_to_toml::json_to_toml;
pub use rustls_provider::ensure_rustls_crypto_provider;
pub use string::approx_bytes_for_tokens;
pub use string::approx_token_count;
pub use string::approx_tokens_from_byte_count;
pub use string::find_uuids;
pub use string::normalize_markdown_hash_location_suffix;
pub use string::sanitize_metric_tag_value;
pub use string::take_bytes_at_char_boundary;
pub use string::truncate_middle_chars;
pub use string::truncate_middle_with_token_budget;
pub use template::Template;
pub use template::TemplateError;
pub use template::TemplateParseError;
pub use template::TemplateRenderError;
pub use template::render;

// Re-export stream-parser items
pub use stream_parser::AssistantTextChunk;
pub use stream_parser::AssistantTextStreamParser;
pub use stream_parser::CitationStreamParser;
pub use stream_parser::ExtractedInlineTag;
pub use stream_parser::InlineHiddenTagParser;
pub use stream_parser::InlineTagSpec;
pub use stream_parser::ProposedPlanParser;
pub use stream_parser::ProposedPlanSegment;
pub use stream_parser::StreamTextChunk;
pub use stream_parser::StreamTextParser;
pub use stream_parser::Utf8StreamParser;
pub use stream_parser::Utf8StreamParserError;
pub use stream_parser::extract_proposed_plan_text;
pub use stream_parser::strip_citations;
pub use stream_parser::strip_proposed_plan_blocks;
