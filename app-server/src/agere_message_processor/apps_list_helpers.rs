use std::sync::Arc;

use agere_app_server_protocol::AppInfo;
use agere_app_server_protocol::AppListUpdatedNotification;
use agere_app_server_protocol::AppsListResponse;
use agere_app_server_protocol::JSONRPCErrorError;
use agere_app_server_protocol::ServerNotification;

use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;

/// Stub: ChatGPT connectors removed. Returns accessible connectors as-is,
/// treating them as the full list since no separate "all connectors" source exists.
#[allow(dead_code)]
pub(super) fn merge_loaded_apps(
    _all_connectors: Option<&[AppInfo]>,
    accessible_connectors: Option<&[AppInfo]>,
) -> Vec<AppInfo> {
    accessible_connectors.map_or_else(Vec::new, <[AppInfo]>::to_vec)
}

#[allow(dead_code)]
pub(super) fn should_send_app_list_updated_notification(
    connectors: &[AppInfo],
    accessible_loaded: bool,
    all_loaded: bool,
) -> bool {
    connectors.iter().any(|connector| connector.is_accessible) || (accessible_loaded && all_loaded)
}

#[allow(dead_code)]
pub(super) fn paginate_apps(
    connectors: &[AppInfo],
    start: usize,
    limit: Option<u32>,
) -> Result<AppsListResponse, JSONRPCErrorError> {
    let total = connectors.len();
    if start > total {
        return Err(JSONRPCErrorError {
            code: INVALID_REQUEST_ERROR_CODE,
            message: format!("cursor {start} exceeds total apps {total}"),
            data: None,
        });
    }

    let effective_limit = limit.unwrap_or(total as u32).max(1) as usize;
    let end = start.saturating_add(effective_limit).min(total);
    let data = connectors[start..end].to_vec();
    let next_cursor = if end < total {
        Some(end.to_string())
    } else {
        None
    };

    Ok(AppsListResponse { data, next_cursor })
}

#[allow(dead_code)]
pub(super) async fn send_app_list_updated_notification(
    outgoing: &Arc<OutgoingMessageSender>,
    data: Vec<AppInfo>,
) {
    outgoing
        .send_server_notification(ServerNotification::AppListUpdated(
            AppListUpdatedNotification { data },
        ))
        .await;
}
