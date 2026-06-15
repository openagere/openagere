use agere_protocol::config_types::WindowsExecutionRestrictionLevel;
use agere_protocol::models::PermissionProfile;
use agere_protocol::permissions::FileSystemAccessPolicy;
use agere_protocol::permissions::FileSystemPath;
use agere_protocol::permissions::FileSystemSpecialPath;
use agere_utils_fs::AbsolutePathBuf;
use async_trait::async_trait;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateDirectoryOptions {
    pub recursive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoveOptions {
    pub recursive: bool,
    pub force: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyOptions {
    pub recursive: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub is_directory: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub created_at_ms: i64,
    pub modified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadDirectoryEntry {
    pub file_name: String,
    pub is_directory: bool,
    pub is_file: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemAccessContext {
    pub permissions: PermissionProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<AbsolutePathBuf>,
    pub windows_execution_restriction_level: WindowsExecutionRestrictionLevel,
    #[serde(default)]
    pub windows_execution_private_desktop: bool,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

impl FileSystemAccessContext {
    pub fn from_permission_profile(permissions: PermissionProfile) -> Self {
        Self::from_permissions_and_cwd(permissions, /*cwd*/ None)
    }

    pub fn from_permission_profile_with_cwd(
        permissions: PermissionProfile,
        cwd: AbsolutePathBuf,
    ) -> Self {
        Self::from_permissions_and_cwd(permissions, Some(cwd))
    }

    fn from_permissions_and_cwd(
        permissions: PermissionProfile,
        cwd: Option<AbsolutePathBuf>,
    ) -> Self {
        Self {
            permissions,
            cwd,
            windows_execution_restriction_level: WindowsExecutionRestrictionLevel::Disabled,
            windows_execution_private_desktop: false,
            use_legacy_landlock: false,
        }
    }

    pub fn has_cwd_dependent_permissions(&self) -> bool {
        let file_system_policy = self.permissions.file_system_access_policy();
        file_system_policy_has_cwd_dependent_entries(&file_system_policy)
    }

    pub fn drop_cwd_if_unused(mut self) -> Self {
        if !self.has_cwd_dependent_permissions() {
            self.cwd = None;
        }
        self
    }
}

fn file_system_policy_has_cwd_dependent_entries(
    file_system_policy: &FileSystemAccessPolicy,
) -> bool {
    file_system_policy
        .entries
        .iter()
        .any(|entry| match &entry.path {
            FileSystemPath::GlobPattern { pattern } => !Path::new(pattern).is_absolute(),
            FileSystemPath::Special {
                value: FileSystemSpecialPath::ProjectRoots { .. },
            } => true,
            FileSystemPath::Path { .. } | FileSystemPath::Special { .. } => false,
        })
}

pub type FileSystemResult<T> = io::Result<T>;

/// Abstract filesystem access used by components that may operate locally or via
/// a remote executor.
#[async_trait]
pub trait ExecutorFileSystem: Send + Sync {
    async fn read_file(&self, path: &AbsolutePathBuf) -> FileSystemResult<Vec<u8>>;

    /// Reads a file and decodes it as UTF-8 text.
    async fn read_file_text(&self, path: &AbsolutePathBuf) -> FileSystemResult<String> {
        let bytes = self.read_file(path).await?;
        String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }

    async fn write_file(&self, path: &AbsolutePathBuf, contents: Vec<u8>) -> FileSystemResult<()>;

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        create_directory_options: CreateDirectoryOptions,
    ) -> FileSystemResult<()>;

    async fn get_metadata(&self, path: &AbsolutePathBuf) -> FileSystemResult<FileMetadata>;

    async fn read_directory(
        &self,
        path: &AbsolutePathBuf,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>>;

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        remove_options: RemoveOptions,
    ) -> FileSystemResult<()>;

    async fn copy(
        &self,
        source_path: &AbsolutePathBuf,
        destination_path: &AbsolutePathBuf,
        copy_options: CopyOptions,
    ) -> FileSystemResult<()>;
}
