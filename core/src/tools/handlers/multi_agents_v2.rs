//! Implements the MultiAgentV2 collaboration tool surface.

use crate::agent::AgentStatus;
use crate::agent::agent_resolver::resolve_agent_target;
use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::multi_agents_common::*;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use agere_protocol::AgentPath;
use agere_protocol::models::ResponseInputItem;
use agere_protocol::openai_models::ReasoningEffort;
use agere_protocol::protocol::CollabAgentInteractionBeginEvent;
use agere_protocol::protocol::CollabAgentInteractionEndEvent;
use agere_protocol::protocol::CollabAgentSpawnBeginEvent;
use agere_protocol::protocol::CollabAgentSpawnEndEvent;
use agere_protocol::protocol::CollabCloseBeginEvent;
use agere_protocol::protocol::CollabCloseEndEvent;
use agere_protocol::protocol::CollabWaitingBeginEvent;
use agere_protocol::protocol::CollabWaitingEndEvent;
use agere_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

pub(crate) use close_agent::Handler as CloseAgentHandler;
pub(crate) use followup_task::Handler as FollowupTaskHandler;
pub(crate) use list_agents::Handler as ListAgentsHandler;
pub(crate) use send_message::Handler as SendMessageHandler;
pub(crate) use spawn::Handler as SpawnAgentHandler;
pub(crate) use wait::Handler as WaitAgentHandler;

mod close_agent;
mod followup_task;
mod list_agents;
mod message_tool;
mod send_message;
mod spawn;
pub(crate) mod wait;
