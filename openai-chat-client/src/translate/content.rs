use crate::types::ChatContent;
use crate::types::ChatContentBlock;
use crate::types::ChatImageUrl;
use agere_protocol::models::ContentItem;
use agere_protocol::models::ImageDetail;

pub(crate) fn content_item_to_chat(item: &ContentItem) -> ChatContent {
    match item {
        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
            ChatContent::Text(text.clone())
        }
        ContentItem::InputImage { image_url, detail } => {
            let detail_str = detail.map(|d| match d {
                ImageDetail::Auto => "auto".into(),
                ImageDetail::Low => "low".into(),
                ImageDetail::High => "high".into(),
                ImageDetail::Original => "high".into(),
            });
            ChatContent::Blocks(vec![ChatContentBlock {
                block_type: "image_url".into(),
                text: None,
                image_url: Some(ChatImageUrl {
                    url: image_url.clone(),
                    detail: detail_str,
                }),
            }])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_protocol::models::ContentItem;

    #[test]
    fn input_text_to_chat_text() {
        let item = ContentItem::InputText {
            text: "Hello".into(),
        };
        let result = content_item_to_chat(&item);
        assert_eq!(result, ChatContent::Text("Hello".into()));
    }

    #[test]
    fn output_text_to_chat_text() {
        let item = ContentItem::OutputText {
            text: "Response".into(),
        };
        let result = content_item_to_chat(&item);
        assert_eq!(result, ChatContent::Text("Response".into()));
    }

    #[test]
    fn input_image_to_chat_image_url() {
        let item = ContentItem::InputImage {
            image_url: "data:image/png;base64,iVBORw0KGgo=".into(),
            detail: None,
        };
        let result = content_item_to_chat(&item);
        match result {
            ChatContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks[0].block_type, "image_url");
                assert_eq!(
                    blocks[0].image_url.as_ref().unwrap().url,
                    "data:image/png;base64,iVBORw0KGgo="
                );
            }
            other => panic!("expected Blocks, got {other:?}"),
        }
    }

    #[test]
    fn input_image_with_detail() {
        use agere_protocol::models::ImageDetail;
        let item = ContentItem::InputImage {
            image_url: "https://example.com/img.png".into(),
            detail: Some(ImageDetail::High),
        };
        let result = content_item_to_chat(&item);
        match result {
            ChatContent::Blocks(blocks) => {
                let img = &blocks[0].image_url.as_ref().unwrap();
                assert_eq!(img.detail.as_deref(), Some("high"));
            }
            other => panic!("expected Blocks, got {other:?}"),
        }
    }
}
