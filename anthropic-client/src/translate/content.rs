use crate::types::ImageSource;
use crate::types::MessageContent;

/// Convert an internal ContentItem to an Anthropic message content block.
pub(crate) fn content_item_to_anthropic(
    item: &agere_protocol::models::ContentItem,
) -> MessageContent {
    match item {
        agere_protocol::models::ContentItem::InputText { text }
        | agere_protocol::models::ContentItem::OutputText { text } => {
            MessageContent::Text { text: text.clone() }
        }
        agere_protocol::models::ContentItem::InputImage { image_url, .. } => {
            let (media_type, data) = parse_data_url(image_url);
            MessageContent::Image {
                source: ImageSource {
                    source_type: "base64".into(),
                    media_type: media_type.into(),
                    data: data.into(),
                },
            }
        }
    }
}

/// Parse a `data:` URL into (media_type, base64_data).
/// Falls back to treating the entire URL as the data if not a data: URL.
fn parse_data_url(url: &str) -> (&str, &str) {
    if let Some(rest) = url.strip_prefix("data:")
        && let Some((media_type, data)) = rest.split_once(";base64,")
    {
        return (media_type, data);
    }
    ("image/png", url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_protocol::models::ContentItem;

    #[test]
    fn input_text_to_anthropic_text_block() {
        let item = ContentItem::InputText {
            text: "Hello".into(),
        };
        let result = content_item_to_anthropic(&item);
        assert_eq!(
            result,
            MessageContent::Text {
                text: "Hello".into()
            }
        );
    }

    #[test]
    fn output_text_to_anthropic_text_block() {
        let item = ContentItem::OutputText {
            text: "Response".into(),
        };
        let result = content_item_to_anthropic(&item);
        assert_eq!(
            result,
            MessageContent::Text {
                text: "Response".into()
            }
        );
    }

    #[test]
    fn input_image_data_url_to_anthropic_image_block() {
        let item = ContentItem::InputImage {
            image_url: "data:image/png;base64,iVBORw0KGgo=".into(),
            detail: None,
        };
        let result = content_item_to_anthropic(&item);
        match result {
            MessageContent::Image { source } => {
                assert_eq!(source.media_type, "image/png");
                assert_eq!(source.data, "iVBORw0KGgo=");
                assert_eq!(source.source_type, "base64");
            }
            other => panic!("expected Image, got {other:?}"),
        }
    }

    #[test]
    fn parse_data_url_without_base64_falls_back() {
        let item = ContentItem::InputImage {
            image_url: "https://example.com/img.png".into(),
            detail: None,
        };
        let result = content_item_to_anthropic(&item);
        match result {
            MessageContent::Image { source } => {
                assert_eq!(source.media_type, "image/png");
                assert_eq!(source.data, "https://example.com/img.png");
            }
            other => panic!("expected Image, got {other:?}"),
        }
    }
}
