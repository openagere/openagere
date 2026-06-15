use super::*;
use agere_protocol::models::AdditionalPermissionProfile;
use agere_protocol::models::FileSystemPermissions;
use agere_protocol::protocol::GranularApprovalConfig;
use core_test_support::PathBufExt;
use pretty_assertions::assert_eq;
use std::collections::HashMap;

#[test]
fn wants_no_access_approval_granular_respects_access_escalation_flag() {
    let runtime = ApplyPatchRuntime::new();
    assert!(runtime.wants_no_access_approval(AskForApproval::OnRequest));
    assert!(
        !runtime.wants_no_access_approval(AskForApproval::Granular(GranularApprovalConfig {
            access_approval: false,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }))
    );
    assert!(
        runtime.wants_no_access_approval(AskForApproval::Granular(GranularApprovalConfig {
            access_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: true,
        }))
    );
}

#[test]
fn guardian_review_request_includes_patch_context() {
    let path = std::env::temp_dir()
        .join("guardian-apply-patch-test.txt")
        .abs();
    let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
    let expected_cwd = action.cwd.clone();
    let expected_patch = action.patch.clone();
    let request = ApplyPatchRequest {
        action,
        file_paths: vec![path.clone()],
        changes: HashMap::from([(
            path.to_path_buf(),
            FileChange::Add {
                content: "hello".to_string(),
            },
        )]),
        exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: None,
        permissions_preapproved: false,
    };

    let guardian_request = ApplyPatchRuntime::build_guardian_review_request(&request, "call-1");

    assert_eq!(
        guardian_request,
        GuardianApprovalRequest::ApplyPatch {
            id: "call-1".to_string(),
            cwd: expected_cwd,
            files: request.file_paths,
            patch: expected_patch,
        }
    );
}

#[test]
fn permission_request_payload_uses_apply_patch_hook_name_and_aliases() {
    let runtime = ApplyPatchRuntime::new();
    let path = std::env::temp_dir()
        .join("apply-patch-permission-request-payload.txt")
        .abs();
    let action = ApplyPatchAction::new_add_for_test(&path, "hello".to_string());
    let expected_patch = action.patch.clone();
    let req = ApplyPatchRequest {
        action,
        file_paths: vec![path],
        changes: HashMap::new(),
        exec_approval_requirement: ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: None,
        permissions_preapproved: false,
    };

    let payload = runtime
        .permission_request_payload(&req)
        .expect("permission request payload");

    assert_eq!(payload.tool_name.name(), "apply_patch");
    assert_eq!(
        payload.tool_name.matcher_aliases(),
        &["Write".to_string(), "Edit".to_string()]
    );
    assert_eq!(
        payload.tool_input,
        serde_json::json!({ "command": expected_patch })
    );
}

#[test]
fn file_system_access_context_is_always_none() {
    let path = std::env::temp_dir()
        .join("apply-patch-runtime-attempt.txt")
        .abs();
    let additional_permissions = AdditionalPermissionProfile {
        network: None,
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![path.clone()]),
            Some(Vec::new()),
        )),
    };
    let req = ApplyPatchRequest {
        action: ApplyPatchAction::new_add_for_test(&path, "hello".to_string()),
        file_paths: vec![path.clone()],
        changes: HashMap::new(),
        exec_approval_requirement: ExecApprovalRequirement::Skip {
            proposed_execpolicy_amendment: None,
        },
        additional_permissions: Some(additional_permissions),
        permissions_preapproved: false,
    };

    // Managed exec layer no longer supplies an apply_patch filesystem context — always None.
    let fs_context = ApplyPatchRuntime::file_system_access_context_for_attempt(&req);
    assert_eq!(fs_context, None);
}
