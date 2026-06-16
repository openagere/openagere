//! Materializes oversized TUI goal objectives, pastes, and images as app-server-host files.

use std::fs;
use std::path::Path;

use crate::app_server_session::AppServerSession;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::LocalImageAttachment;
use agere_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use agere_protocol::user_input::TextElement;
use agere_utils_fs::AbsolutePathBuf;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use uuid::Uuid;

const GOAL_ATTACHMENT_DIR: &str = "attachments";
const GOAL_FILE_PREFIX: &str = "Read the Agere goal objective file at ";
const GOAL_FILE_SUFFIX: &str = " before continuing.";
const GOAL_FILE_NAME: &str = "goal-objective.md";

#[derive(Clone, Debug, Default)]
pub(crate) struct GoalDraft {
    pub(crate) objective: String,
    pub(crate) text_elements: Vec<TextElement>,
    pub(crate) pending_pastes: Vec<(String, String)>,
    pub(crate) local_images: Vec<LocalImageAttachment>,
    pub(crate) remote_image_urls: Vec<String>,
}

pub(crate) type GoalFilePath = AbsolutePathBuf;

pub(crate) async fn materialize_goal_draft(
    app_server: &mut AppServerSession,
    agere_home: &Path,
    draft: GoalDraft,
) -> Result<(String, Option<GoalFilePath>)> {
    let mut objective = draft.objective;
    if objective.trim().is_empty() {
        bail!("Goal objective must not be empty.");
    }
    let text_elements = draft.text_elements;
    if !draft.pending_pastes.is_empty() {
        let (expanded_objective, _) = ChatComposer::expand_pending_pastes(
            &objective,
            text_elements.clone(),
            &draft.pending_pastes,
        );
        if expanded_objective.trim().is_empty() {
            bail!("Goal objective must not be empty.");
        }
    }

    let mut active_placeholders = text_elements
        .iter()
        .filter_map(|element| element.placeholder(&objective))
        .filter(|placeholder| !placeholder.is_empty())
        .collect::<Vec<_>>();
    let mut output_dir = None;
    let mut replacements = Vec::new();
    for (placeholder, text) in draft.pending_pastes.iter() {
        let Some(active_idx) = active_placeholders
            .iter()
            .position(|active| *active == placeholder.as_str())
        else {
            continue;
        };
        active_placeholders.swap_remove(active_idx);
        let path = ensure_goal_output_dir(app_server, agere_home, &mut output_dir)
            .await?
            .join(format!("pasted-text-{}.txt", replacements.len() + 1));
        write_goal_file(app_server, path.clone(), text.as_bytes().to_vec()).await?;

        replacements.push((
            placeholder.clone(),
            format!(
                "pasted text file: {}. Read this file before continuing.",
                path_display(&path)
            ),
        ));
    }

    let mut image_lines = Vec::new();
    for (idx, image) in draft.local_images.iter().enumerate() {
        if !image.placeholder.is_empty() {
            let Some(active_idx) = active_placeholders
                .iter()
                .position(|active| *active == image.placeholder.as_str())
            else {
                continue;
            };
            active_placeholders.swap_remove(active_idx);
        }
        let extension = image_extension(&image.path);
        let path = ensure_goal_output_dir(app_server, agere_home, &mut output_dir)
            .await?
            .join(format!("image-{}.{}", idx + 1, extension));
        let bytes = fs::read(&image.path)
            .with_context(|| format!("Could not read goal image {}", image.path.display()))?;
        write_goal_file(app_server, path.clone(), bytes).await?;
        if image.placeholder.is_empty() {
            image_lines.push(format!("- [Image #{}]: {}", idx + 1, path_display(&path)));
        } else {
            replacements.push((
                image.placeholder.clone(),
                format!("image file: {}", path_display(&path)),
            ));
        }
    }

    let (expanded_objective, _) =
        ChatComposer::expand_pending_pastes(&objective, text_elements, &replacements);
    objective = expanded_objective.trim().to_string();
    append_section(&mut objective, "Referenced image files:", image_lines);

    append_section(
        &mut objective,
        "Referenced image URLs:",
        draft
            .remote_image_urls
            .into_iter()
            .enumerate()
            .map(|(idx, url)| format!("- [Image #{}]: {url}", idx + 1))
            .collect(),
    );

    if objective.chars().count() > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        let path = ensure_goal_output_dir(app_server, agere_home, &mut output_dir)
            .await?
            .join(GOAL_FILE_NAME);
        let reference = match objective_file_reference(&path) {
            Ok(reference) => reference,
            Err(err) => {
                if let Some(output_dir) = output_dir
                    && let Err(cleanup_err) = app_server.fs_remove_path(&output_dir).await
                {
                    tracing::warn!(
                        "failed to clean up materialized goal files at {}: {cleanup_err}",
                        path_display(&output_dir)
                    );
                }
                return Err(err);
            }
        };
        write_goal_file(app_server, path, objective.as_bytes().to_vec()).await?;
        objective = reference;
    }
    Ok((objective, output_dir))
}

pub(crate) async fn objective_text_for_edit(
    app_server: &mut AppServerSession,
    agere_home: &Path,
    objective: &str,
) -> Result<String> {
    let Some(path) = objective_file_path(objective, agere_home) else {
        return Ok(objective.to_string());
    };
    let bytes = app_server
        .fs_read_file_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not read goal objective file {}", path_display(&path)))?;
    String::from_utf8(bytes).with_context(|| {
        format!(
            "Goal objective file {} is not valid UTF-8",
            path_display(&path)
        )
    })
}

pub(crate) fn objective_file_path(objective: &str, agere_home: &Path) -> Option<GoalFilePath> {
    let path = objective
        .strip_prefix(GOAL_FILE_PREFIX)
        .and_then(|path| path.strip_suffix(GOAL_FILE_SUFFIX))?;
    let path = AbsolutePathBuf::try_from(Path::new(path).to_path_buf()).ok()?;
    let attachment_root = agere_home.join(GOAL_ATTACHMENT_DIR);
    let relative = path.strip_prefix(&attachment_root).ok()?;
    let mut components = relative.components();
    let attachment_id = components.next()?.as_os_str().to_str()?;
    (components.next()?.as_os_str() == GOAL_FILE_NAME
        && components.next().is_none()
        && Uuid::parse_str(attachment_id).is_ok())
    .then_some(path)
}

pub(crate) fn objective_file_reference(path: &GoalFilePath) -> Result<String> {
    let reference = format!("{GOAL_FILE_PREFIX}{}{GOAL_FILE_SUFFIX}", path_display(path));
    let actual_chars = reference.chars().count();
    if actual_chars > MAX_THREAD_GOAL_OBJECTIVE_CHARS {
        bail!(
            "Goal objective file reference is too long: {actual_chars} characters. Limit: {MAX_THREAD_GOAL_OBJECTIVE_CHARS} characters."
        );
    }
    Ok(reference)
}

async fn ensure_goal_output_dir(
    app_server: &mut AppServerSession,
    agere_home: &Path,
    output_dir: &mut Option<GoalFilePath>,
) -> Result<GoalFilePath> {
    if let Some(output_dir) = output_dir {
        return Ok(output_dir.clone());
    }
    let path = AbsolutePathBuf::try_from(
        agere_home
            .join(GOAL_ATTACHMENT_DIR)
            .join(Uuid::new_v4().to_string()),
    )
    .context("Agere home must be an absolute path to materialize goal files")?;
    app_server
        .fs_create_directory_all_path(&path)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| {
            format!(
                "Could not create goal attachment directory {}",
                path_display(&path)
            )
        })?;
    *output_dir = Some(path.clone());
    Ok(path)
}

async fn write_goal_file(
    app_server: &mut AppServerSession,
    path: GoalFilePath,
    bytes: Vec<u8>,
) -> Result<()> {
    app_server
        .fs_write_file_path(&path, bytes)
        .await
        .map_err(|err| anyhow::anyhow!("{err}"))
        .with_context(|| format!("Could not write goal file {}", path_display(&path)))
}

fn path_display(path: &GoalFilePath) -> String {
    path.as_path().display().to_string()
}

fn append_section(objective: &mut String, heading: &str, lines: Vec<String>) {
    if lines.is_empty() {
        return;
    }
    if !objective.ends_with('\n') {
        objective.push_str("\n\n");
    }
    objective.push_str(heading);
    objective.push('\n');
    objective.push_str(&lines.join("\n"));
}

fn image_extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(8)
                .collect::<String>()
        })
        .filter(|extension| !extension.is_empty())
        .unwrap_or_else(|| "png".to_string())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agere_utils_fs::test_support::PathBufExt;

    use super::*;

    #[test]
    fn objective_file_reference_round_trips_valid_attachment_path() {
        let agere_home = PathBuf::from("D:/agere-home").abs();
        let path = agere_home
            .join(GOAL_ATTACHMENT_DIR)
            .join("550e8400-e29b-41d4-a716-446655440000")
            .join(GOAL_FILE_NAME);

        let reference = objective_file_reference(&path).expect("reference should fit");

        assert_eq!(
            objective_file_path(&reference, agere_home.as_path()),
            Some(path)
        );
    }

    #[test]
    fn objective_file_path_rejects_paths_outside_goal_attachments() {
        let agere_home = PathBuf::from("D:/agere-home").abs();
        let outside_path = agere_home
            .join("other")
            .join("550e8400-e29b-41d4-a716-446655440000")
            .join(GOAL_FILE_NAME);
        let reference = format!(
            "{GOAL_FILE_PREFIX}{}{GOAL_FILE_SUFFIX}",
            outside_path.display()
        );

        assert_eq!(objective_file_path(&reference, agere_home.as_path()), None);
    }

    #[test]
    fn objective_file_path_rejects_unrecognized_attachment_shape() {
        let agere_home = PathBuf::from("D:/agere-home").abs();
        for path in [
            agere_home
                .join(GOAL_ATTACHMENT_DIR)
                .join("not-a-uuid")
                .join(GOAL_FILE_NAME),
            agere_home
                .join(GOAL_ATTACHMENT_DIR)
                .join("550e8400-e29b-41d4-a716-446655440000")
                .join("other.md"),
            agere_home
                .join(GOAL_ATTACHMENT_DIR)
                .join("550e8400-e29b-41d4-a716-446655440000")
                .join(GOAL_FILE_NAME)
                .join("nested"),
        ] {
            let reference = format!("{GOAL_FILE_PREFIX}{}{GOAL_FILE_SUFFIX}", path.display());
            assert_eq!(objective_file_path(&reference, agere_home.as_path()), None);
        }
    }

    #[test]
    fn image_extension_sanitizes_untrusted_extension_text() {
        assert_eq!(image_extension(Path::new("image.pn-g")), "png");
        assert_eq!(
            image_extension(Path::new("image.longextension")),
            "longexte"
        );
        assert_eq!(image_extension(Path::new("image")), "png");
    }
}
