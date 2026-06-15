#![cfg(unix)]

mod common;

use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use agere_exec_server::CopyOptions;
use agere_exec_server::CreateDirectoryOptions;
use agere_exec_server::Environment;
use agere_exec_server::ExecServerRuntimePaths;
use agere_exec_server::ExecutorFileSystem;
use agere_exec_server::LocalFileSystem;
use agere_exec_server::ReadDirectoryEntry;
use agere_exec_server::RemoveOptions;
use agere_utils_fs::AbsolutePathBuf;
use anyhow::Context;
use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;

use common::exec_server::ExecServerHarness;
use common::exec_server::TestAgereHelperPaths;
use common::exec_server::exec_server;
use common::exec_server::test_agere_helper_paths;

struct FileSystemContext {
    file_system: Arc<dyn ExecutorFileSystem>,
    _helper_paths: Option<TestAgereHelperPaths>,
    _server: Option<ExecServerHarness>,
}

async fn create_file_system_context(use_remote: bool) -> Result<FileSystemContext> {
    if use_remote {
        let server = exec_server().await?;
        let environment = Environment::create_for_tests(Some(server.websocket_url().to_string()))?;
        Ok(FileSystemContext {
            file_system: environment.get_filesystem(),
            _helper_paths: None,
            _server: Some(server),
        })
    } else {
        let helper_paths = test_agere_helper_paths()?;
        let runtime_paths = ExecServerRuntimePaths::new(
            helper_paths.agere_exe.clone(),
            helper_paths.agere_linux_exe.clone(),
        )?;
        Ok(FileSystemContext {
            file_system: Arc::new(LocalFileSystem::with_runtime_paths(runtime_paths)),
            _helper_paths: Some(helper_paths),
            _server: None,
        })
    }
}

fn absolute_path(path: std::path::PathBuf) -> AbsolutePathBuf {
    assert!(
        path.is_absolute(),
        "path must be absolute: {}",
        path.display()
    );
    match AbsolutePathBuf::try_from(path) {
        Ok(path) => path,
        Err(err) => panic!("path should be absolute: {err}"),
    }
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_get_metadata_returns_expected_fields(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let file_path = tmp.path().join("note.txt");
    std::fs::write(&file_path, "hello")?;

    let metadata = file_system
        .get_metadata(&absolute_path(file_path.clone()))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(metadata.is_directory, false);
    assert_eq!(metadata.is_file, true);
    assert_eq!(metadata.is_symlink, false);
    assert!(metadata.modified_at_ms > 0);

    let symlink_path = tmp.path().join("note-link.txt");
    symlink(&file_path, &symlink_path)?;
    let symlink_metadata = file_system
        .get_metadata(&absolute_path(symlink_path.clone()))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(symlink_metadata.is_directory, false);
    assert_eq!(symlink_metadata.is_file, true);
    assert_eq!(symlink_metadata.is_symlink, true);
    assert!(symlink_metadata.modified_at_ms > 0);

    let dir_path = tmp.path().join("notes");
    std::fs::create_dir(&dir_path)?;
    let dir_symlink_path = tmp.path().join("notes-link");
    symlink(&dir_path, &dir_symlink_path)?;
    let dir_symlink_metadata = file_system
        .get_metadata(&absolute_path(dir_symlink_path))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(dir_symlink_metadata.is_directory, true);
    assert_eq!(dir_symlink_metadata.is_file, false);
    assert_eq!(dir_symlink_metadata.is_symlink, true);

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_methods_cover_surface_area(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let source_file = source_dir.join("root.txt");
    let nested_file = nested_dir.join("note.txt");
    let copied_dir = tmp.path().join("copied");
    let copied_file = tmp.path().join("copy.txt");

    file_system
        .create_directory(
            &absolute_path(nested_dir.clone()),
            CreateDirectoryOptions { recursive: true },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    file_system
        .write_file(
            &absolute_path(nested_file.clone()),
            b"hello from trait".to_vec(),
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    file_system
        .write_file(
            &absolute_path(source_file.clone()),
            b"hello from source root".to_vec(),
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    let nested_file_contents = file_system
        .read_file(&absolute_path(nested_file.clone()))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(nested_file_contents, b"hello from trait");

    let nested_file_text = file_system
        .read_file_text(&absolute_path(nested_file.clone()))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(nested_file_text, "hello from trait");

    file_system
        .copy(
            &absolute_path(nested_file),
            &absolute_path(copied_file.clone()),
            CopyOptions { recursive: false },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(std::fs::read_to_string(copied_file)?, "hello from trait");

    file_system
        .copy(
            &absolute_path(source_dir.clone()),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert_eq!(
        std::fs::read_to_string(copied_dir.join("nested").join("note.txt"))?,
        "hello from trait"
    );

    symlink(
        source_dir.join("missing-target"),
        source_dir.join("broken-link"),
    )?;

    let mut entries = file_system
        .read_directory(&absolute_path(source_dir))
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    entries.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    assert_eq!(
        entries,
        vec![
            ReadDirectoryEntry {
                file_name: "nested".to_string(),
                is_directory: true,
                is_file: false,
            },
            ReadDirectoryEntry {
                file_name: "root.txt".to_string(),
                is_directory: false,
                is_file: true,
            },
        ]
    );

    file_system
        .remove(
            &absolute_path(copied_dir.clone()),
            RemoveOptions {
                recursive: true,
                force: true,
            },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;
    assert!(!copied_dir.exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_write_file_reports_missing_parent(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let missing_parent_path = tmp.path().join("missing").join("note.txt");

    let error = match file_system
        .write_file(
            &absolute_path(missing_parent_path.clone()),
            b"hello from trait".to_vec(),
        )
        .await
    {
        Ok(()) => anyhow::bail!("write should fail when parent directory is absent"),
        Err(error) => error,
    };
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "mode={use_remote}"
    );
    assert!(!missing_parent_path.exists(), "mode={use_remote}");

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_directory_without_recursive(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(&source_dir)?;

    let error = file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(tmp.path().join("dest")),
            CopyOptions { recursive: false },
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy requires recursive: true when sourcePath is a directory"
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_copying_directory_into_descendant(
    use_remote: bool,
) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    std::fs::create_dir_all(source_dir.join("nested"))?;

    let error = file_system
        .copy(
            &absolute_path(source_dir.clone()),
            &absolute_path(source_dir.join("nested").join("copy")),
            CopyOptions { recursive: true },
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy cannot copy a directory to itself or one of its descendants"
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_preserves_symlinks_in_recursive_copy(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let nested_dir = source_dir.join("nested");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&nested_dir)?;
    symlink("nested", source_dir.join("nested-link"))?;

    file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    let copied_link = copied_dir.join("nested-link");
    let metadata = std::fs::symlink_metadata(&copied_link)?;
    assert!(metadata.file_type().is_symlink());
    assert_eq!(
        std::fs::read_link(copied_link)?,
        std::path::PathBuf::from("nested")
    );

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_ignores_unknown_special_files_in_recursive_copy(
    use_remote: bool,
) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let source_dir = tmp.path().join("source");
    let copied_dir = tmp.path().join("copied");
    std::fs::create_dir_all(&source_dir)?;
    std::fs::write(source_dir.join("note.txt"), "hello")?;

    let fifo_path = source_dir.join("named-pipe");
    let output = Command::new("mkfifo").arg(&fifo_path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    file_system
        .copy(
            &absolute_path(source_dir),
            &absolute_path(copied_dir.clone()),
            CopyOptions { recursive: true },
        )
        .await
        .with_context(|| format!("mode={use_remote}"))?;

    assert_eq!(
        std::fs::read_to_string(copied_dir.join("note.txt"))?,
        "hello"
    );
    assert!(!copied_dir.join("named-pipe").exists());

    Ok(())
}

#[test_case(false ; "local")]
#[test_case(true ; "remote")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_system_copy_rejects_standalone_fifo_source(use_remote: bool) -> Result<()> {
    let context = create_file_system_context(use_remote).await?;
    let file_system = context.file_system;

    let tmp = TempDir::new()?;
    let fifo_path = tmp.path().join("named-pipe");
    let output = Command::new("mkfifo").arg(&fifo_path).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "mkfifo failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let error = file_system
        .copy(
            &absolute_path(fifo_path),
            &absolute_path(tmp.path().join("copied")),
            CopyOptions { recursive: false },
        )
        .await;
    let error = match error {
        Ok(()) => panic!("copy should fail"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        error.to_string(),
        "fs/copy only supports regular files, directories, and symlinks"
    );

    Ok(())
}
