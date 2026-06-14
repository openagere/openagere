use crate::config::Config;
pub use agere_rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use agere_rollout::Cursor;
pub use agere_rollout::EventPersistenceMode;
pub use agere_rollout::INTERACTIVE_SESSION_SOURCES;
pub use agere_rollout::RolloutRecorder;
pub use agere_rollout::RolloutRecorderParams;
pub use agere_rollout::SESSIONS_SUBDIR;
pub use agere_rollout::SessionMeta;
pub use agere_rollout::SortDirection;
pub use agere_rollout::ThreadItem;
pub use agere_rollout::ThreadSortKey;
pub use agere_rollout::ThreadsPage;
pub use agere_rollout::append_thread_name;
pub use agere_rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use agere_rollout::find_conversation_path_by_id_str;
pub use agere_rollout::find_thread_meta_by_name_str;
pub use agere_rollout::find_thread_name_by_id;
pub use agere_rollout::find_thread_names_by_ids;
pub use agere_rollout::find_thread_path_by_id_str;
pub use agere_rollout::parse_cursor;
pub use agere_rollout::read_head_for_summary;
pub use agere_rollout::read_session_meta_line;
pub use agere_rollout::rollout_date_parts;

impl agere_rollout::RolloutConfigView for Config {
    fn agere_home(&self) -> &std::path::Path {
        self.agere_home.as_path()
    }

    fn sqlite_home(&self) -> &std::path::Path {
        self.sqlite_home.as_path()
    }

    fn cwd(&self) -> &std::path::Path {
        self.cwd.as_path()
    }

    fn model_provider_id(&self) -> &str {
        self.model_provider_id.as_str()
    }

    fn generate_memories(&self) -> bool {
        self.memories.generate_memories
    }
}

pub(crate) mod list {
    pub use agere_rollout::find_thread_path_by_id_str;
}

pub(crate) mod recorder {
    pub use agere_rollout::RolloutRecorder;
}

pub(crate) use crate::session_rollout_init_error::map_session_init_error;

pub(crate) mod truncation {
    pub(crate) use crate::thread_rollout_truncation::*;
}
