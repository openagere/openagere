//! Filesystem utilities: absolute paths, home directory resolution, and path operations.

pub mod absolute_path;
pub mod home_dir;
pub mod path_utils;

mod env;
pub use env::is_wsl;

// Re-export the main type at the top level for convenience
pub use absolute_path::AbsolutePathBuf;
pub use absolute_path::AbsolutePathBufGuard;
pub use absolute_path::canonicalize_existing_preserving_symlinks;
pub use absolute_path::canonicalize_preserving_symlinks;

// Re-export test support
pub use absolute_path::test_support;

// Re-export path_utils items
pub use path_utils::SymlinkWritePaths;
pub use path_utils::normalize_for_native_workdir;
pub use path_utils::normalize_for_path_comparison;
pub use path_utils::paths_match_after_normalization;
pub use path_utils::resolve_symlink_write_paths;
pub use path_utils::write_atomically;

// Re-export home_dir items
pub use home_dir::find_agere_home;
