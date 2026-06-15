use super::ContextualUserFragment;
use agere_execpolicy::Policy;
use agere_protocol::config_types::AccessMode;
use agere_protocol::config_types::ApprovalsReviewer;
use agere_protocol::models::PermissionProfile;
use agere_protocol::models::format_allow_prefixes;
use agere_protocol::permissions::NetworkAccessPolicy;
use agere_protocol::protocol::AskForApproval;
use agere_protocol::protocol::GranularApprovalConfig;
use agere_protocol::protocol::NetworkAccess;
use agere_protocol::protocol::WritableRoot;
use agere_utils_common::Template;
use std::path::Path;
use std::sync::LazyLock;

const APPROVAL_POLICY_NEVER: &str = include_str!("prompts/permissions/approval_policy/never.md");
const APPROVAL_POLICY_UNLESS_TRUSTED: &str =
    include_str!("prompts/permissions/approval_policy/unless_trusted.md");
const APPROVAL_POLICY_ON_FAILURE: &str =
    include_str!("prompts/permissions/approval_policy/on_failure.md");
const APPROVAL_POLICY_ON_REQUEST_RULE: &str =
    include_str!("prompts/permissions/approval_policy/on_request.md");
const APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION: &str =
    include_str!("prompts/permissions/approval_policy/on_request_rule_request_permission.md");
const AUTO_REVIEW_APPROVAL_SUFFIX: &str = "`approvals_reviewer` is `auto_review`: Access escalations with require_escalated will be reviewed for compliance with the policy. If a rejection happens, you should proceed only with a materially safer alternative, or inform the user of the risk and send a final message to ask for approval.";

const ACCESS_MODE_DANGER_FULL_ACCESS: &str =
    include_str!("prompts/permissions/access_mode/danger_full_access.md");
const ACCESS_MODE_WORKSPACE_WRITE: &str =
    include_str!("prompts/permissions/access_mode/workspace_write.md");
const ACCESS_MODE_READ_ONLY: &str = include_str!("prompts/permissions/access_mode/read_only.md");

static ACCESS_MODE_DANGER_FULL_ACCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(ACCESS_MODE_DANGER_FULL_ACCESS.trim_end())
        .unwrap_or_else(|err| panic!("danger-full-access access mode template must parse: {err}"))
});
static ACCESS_MODE_WORKSPACE_WRITE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(ACCESS_MODE_WORKSPACE_WRITE.trim_end())
        .unwrap_or_else(|err| panic!("workspace-write access mode template must parse: {err}"))
});
static ACCESS_MODE_READ_ONLY_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
    Template::parse(ACCESS_MODE_READ_ONLY.trim_end())
        .unwrap_or_else(|err| panic!("read-only access mode template must parse: {err}"))
});

struct PermissionsPromptConfig<'a> {
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    exec_policy: &'a Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
/// Developer instructions that describe filesystem access constraints and approval policy.
pub struct PermissionsInstructions {
    text: String,
}

impl PermissionsInstructions {
    /// Builds permissions instructions from the effective permission profile and approval policy.
    pub fn from_permission_profile(
        permission_profile: &PermissionProfile,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let (access_mode, writable_roots) =
            access_mode_prompt_from_permission_profile(permission_profile, cwd);

        Self::from_permissions_with_network(
            access_mode,
            network_access_from_policy(permission_profile.network_access_policy()),
            PermissionsPromptConfig {
                approval_policy,
                approvals_reviewer,
                exec_policy,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            },
            writable_roots,
        )
    }

    /// Builds permissions instructions from a legacy access policy string.
    pub fn from_policy(
        access_policy: &str,
        approval_policy: AskForApproval,
        approvals_reviewer: ApprovalsReviewer,
        exec_policy: &Policy,
        cwd: &Path,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        Self::from_permission_profile(
            &PermissionProfile::from_legacy_access_policy(access_policy),
            approval_policy,
            approvals_reviewer,
            exec_policy,
            cwd,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
        )
    }

    fn from_permissions_with_network(
        access_mode: AccessMode,
        network_access: NetworkAccess,
        config: PermissionsPromptConfig<'_>,
        writable_roots: Option<Vec<WritableRoot>>,
    ) -> Self {
        let mut text = String::new();
        append_section(&mut text, &access_mode_text(access_mode, network_access));
        append_section(
            &mut text,
            &approval_text(
                config.approval_policy,
                config.approvals_reviewer,
                config.exec_policy,
                config.exec_permission_approvals_enabled,
                config.request_permissions_tool_enabled,
            ),
        );
        if let Some(writable_roots) = writable_roots_text(writable_roots) {
            append_section(&mut text, &writable_roots);
        }
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Self { text }
    }
}

fn access_mode_prompt_from_permission_profile(
    permission_profile: &PermissionProfile,
    cwd: &Path,
) -> (AccessMode, Option<Vec<WritableRoot>>) {
    match permission_profile {
        PermissionProfile::Disabled | PermissionProfile::External { .. } => {
            (AccessMode::DangerFullAccess, None)
        }
        PermissionProfile::Managed { .. } => {
            let file_system_policy = permission_profile.file_system_access_policy();
            if file_system_policy.has_full_disk_write_access() {
                return (AccessMode::DangerFullAccess, None);
            }

            let writable_roots = file_system_policy.get_writable_roots_with_cwd(cwd);
            if writable_roots.is_empty() {
                (AccessMode::ReadOnly, None)
            } else {
                (AccessMode::WorkspaceWrite, Some(writable_roots))
            }
        }
    }
}

fn network_access_from_policy(network_policy: NetworkAccessPolicy) -> NetworkAccess {
    if network_policy.is_enabled() {
        NetworkAccess::Enabled
    } else {
        NetworkAccess::Restricted
    }
}

impl ContextualUserFragment for PermissionsInstructions {
    const ROLE: &'static str = "developer";
    const START_MARKER: &'static str = "<permissions instructions>";
    const END_MARKER: &'static str = "</permissions instructions>";

    fn body(&self) -> String {
        self.text.clone()
    }
}

fn append_section(text: &mut String, section: &str) {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(section);
}

fn approval_text(
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
    exec_policy: &Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let with_request_permissions_tool = |text: &str| {
        if request_permissions_tool_enabled {
            format!("{text}\n\n{}", request_permissions_tool_prompt_section())
        } else {
            text.to_string()
        }
    };
    let on_request_instructions = || {
        let on_request_rule = if exec_permission_approvals_enabled {
            APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string()
        } else {
            APPROVAL_POLICY_ON_REQUEST_RULE.to_string()
        };
        let mut sections = vec![on_request_rule];
        if request_permissions_tool_enabled {
            sections.push(request_permissions_tool_prompt_section().to_string());
        }
        if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
            sections.push(format!(
                "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
            ));
        }
        sections.join("\n\n")
    };
    let text = match approval_policy {
        AskForApproval::Never => APPROVAL_POLICY_NEVER.to_string(),
        AskForApproval::UnlessTrusted => {
            with_request_permissions_tool(APPROVAL_POLICY_UNLESS_TRUSTED)
        }
        AskForApproval::OnFailure => with_request_permissions_tool(APPROVAL_POLICY_ON_FAILURE),
        AskForApproval::OnRequest => on_request_instructions(),
        AskForApproval::Granular(granular_config) => granular_instructions(
            granular_config,
            exec_policy,
            exec_permission_approvals_enabled,
            request_permissions_tool_enabled,
        ),
    };

    if approvals_reviewer == ApprovalsReviewer::AutoReview
        && approval_policy != AskForApproval::Never
    {
        format!("{text}\n\n{AUTO_REVIEW_APPROVAL_SUFFIX}")
    } else {
        text
    }
}

fn access_mode_text(mode: AccessMode, network_access: NetworkAccess) -> String {
    let template = match mode {
        AccessMode::DangerFullAccess => &*ACCESS_MODE_DANGER_FULL_ACCESS_TEMPLATE,
        AccessMode::WorkspaceWrite => &*ACCESS_MODE_WORKSPACE_WRITE_TEMPLATE,
        AccessMode::ReadOnly => &*ACCESS_MODE_READ_ONLY_TEMPLATE,
    };
    let network_access = network_access.to_string();
    template
        .render([("network_access", network_access.as_str())])
        .unwrap_or_else(|err| panic!("access mode template must render: {err}"))
}

fn writable_roots_text(writable_roots: Option<Vec<WritableRoot>>) -> Option<String> {
    let roots = writable_roots?;
    if roots.is_empty() {
        return None;
    }

    let roots_list: Vec<String> = roots
        .iter()
        .map(|r| format!("`{}`", r.root.to_string_lossy()))
        .collect();
    Some(if roots_list.len() == 1 {
        format!(" The writable root is {}.", roots_list[0])
    } else {
        format!(" The writable roots are {}.", roots_list.join(", "))
    })
}

fn approved_command_prefixes_text(exec_policy: &Policy) -> Option<String> {
    format_allow_prefixes(exec_policy.get_allowed_prefixes())
        .filter(|prefixes| !prefixes.is_empty())
}

fn granular_prompt_intro_text() -> &'static str {
    "# Approval Requests\n\nApproval policy is `granular`. Categories set to `false` are automatically rejected instead of prompting the user."
}

fn request_permissions_tool_prompt_section() -> &'static str {
    "# request_permissions Tool\n\nThe built-in `request_permissions` tool is available in this session. Invoke it when you need to request additional `network` or `file_system` permissions before later shell-like commands need them. Request only the specific permissions required for the task."
}

fn granular_instructions(
    granular_config: GranularApprovalConfig,
    exec_policy: &Policy,
    exec_permission_approvals_enabled: bool,
    request_permissions_tool_enabled: bool,
) -> String {
    let access_approval_prompts_allowed = granular_config.allows_access_approval();
    let shell_permission_requests_available =
        exec_permission_approvals_enabled && access_approval_prompts_allowed;
    let request_permissions_tool_prompts_allowed =
        request_permissions_tool_enabled && granular_config.allows_request_permissions();
    let categories = [
        Some((
            granular_config.allows_access_approval(),
            "`access_approval`",
        )),
        Some((granular_config.allows_rules_approval(), "`rules`")),
        Some((granular_config.allows_skill_approval(), "`skill_approval`")),
        request_permissions_tool_enabled.then_some((
            granular_config.allows_request_permissions(),
            "`request_permissions`",
        )),
        Some((
            granular_config.allows_mcp_elicitations(),
            "`mcp_elicitations`",
        )),
    ];
    let prompted_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();
    let rejected_categories = categories
        .iter()
        .flatten()
        .filter(|&&(is_allowed, _)| !is_allowed)
        .map(|&(_, category)| format!("- {category}"))
        .collect::<Vec<_>>();

    let mut sections = vec![granular_prompt_intro_text().to_string()];

    if !prompted_categories.is_empty() {
        sections.push(format!(
            "These approval categories may still prompt the user when needed:\n{}",
            prompted_categories.join("\n")
        ));
    }
    if !rejected_categories.is_empty() {
        sections.push(format!(
            "These approval categories are automatically rejected instead of prompting the user:\n{}",
            rejected_categories.join("\n")
        ));
    }

    if shell_permission_requests_available {
        sections.push(APPROVAL_POLICY_ON_REQUEST_RULE_REQUEST_PERMISSION.to_string());
    }

    if request_permissions_tool_prompts_allowed {
        sections.push(request_permissions_tool_prompt_section().to_string());
    }

    if let Some(prefixes) = approved_command_prefixes_text(exec_policy) {
        sections.push(format!(
            "## Approved command prefixes\nThe following prefix rules have already been approved: {prefixes}"
        ));
    }

    sections.join("\n\n")
}

#[cfg(test)]
#[path = "permissions_instructions_tests.rs"]
mod permissions_instructions_tests;
