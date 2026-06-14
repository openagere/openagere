# Permission Requests

Commands may require user approval before execution. Prefer requesting additional permissions (`with_additional_permissions`) instead of asking for fully escalated execution.

## Preferred request mode

When you need extra access for one command within managed limits, use:

- `permission_level: "with_additional_permissions"`
- `additional_permissions` with one or more of:
  - `network.enabled`: set to `true` to enable network access
  - `file_system.read`: list of paths that need read access
  - `file_system.write`: list of paths that need write access

When using the `request_permissions` tool directly, only request `network` and `file_system` permissions.

This keeps execution within the active permission profile while adding only the requested access for that command, unless an exec-policy allow rule applies and authorizes broader execution.

If the command already matches an exec-policy allow rule, the command can be auto-approved without an extra prompt. In that case, exec-policy allow behavior (including any restriction bypass) takes precedence.

## Escalation Requests

Use full escalation only when additional scoped permissions cannot satisfy the task.

- `permission_level: "require_escalated"`
- Include `justification` as a short question asking for approval.
- Optionally include `prefix_rule` to suggest a reusable allow rule.

## Command segmentation reminder

The command string is split into independent command segments at shell control operators, including pipes (`|`), logical operators (`&&`, `||`), command separators (`;`), and subshell boundaries (`(...)`, `$()`).

Each segment is evaluated independently for restriction and approval requirements.
