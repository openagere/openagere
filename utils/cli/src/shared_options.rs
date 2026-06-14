//! Shared command-line flags used by both interactive and non-interactive Agere entry points.

use agere_protocol::config_types::AccessMode;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Default)]
pub struct SharedCliOptions {
    /// Optional image(s) to attach to the initial prompt.
    #[arg(
        long = "image",
        short = 'i',
        value_name = "FILE",
        value_delimiter = ',',
        num_args = 1..
    )]
    pub images: Vec<PathBuf>,

    /// Model the agent should use.
    #[arg(long, short = 'm')]
    pub model: Option<String>,

    /// Configuration profile from config.toml to specify default options.
    #[arg(long = "profile", short = 'p')]
    pub config_profile: Option<String>,

    /// Select the access mode to use when executing model-generated shell
    /// commands.
    #[arg(long = "access-mode")]
    pub access_mode: Option<AccessMode>,

    /// Convenience alias for low-friction automatic execution.
    #[arg(long = "full-auto", default_value_t = false)]
    pub full_auto: bool,

    /// Skip all confirmation prompts and execute commands without access restrictions.
    /// EXTREMELY DANGEROUS. Intended solely for environments that enforce isolation externally.
    #[arg(
        long = "dangerously-bypass-approvals",
        alias = "yolo",
        default_value_t = false,
        conflicts_with = "full_auto"
    )]
    pub dangerously_bypass_approvals: bool,

    /// Tell the agent to use the specified directory as its working root.
    #[clap(long = "cd", short = 'C', value_name = "DIR")]
    pub cwd: Option<PathBuf>,

    /// Additional directories that should be writable alongside the primary workspace.
    #[arg(long = "add-dir", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    pub add_dir: Vec<PathBuf>,
}

impl SharedCliOptions {
    pub fn inherit_exec_root_options(&mut self, root: &Self) {
        let self_selected_access_mode =
            self.access_mode.is_some() || self.full_auto || self.dangerously_bypass_approvals;
        let Self {
            images,
            model,
            config_profile,
            access_mode,
            full_auto,
            dangerously_bypass_approvals,
            cwd,
            add_dir,
        } = self;
        let Self {
            images: root_images,
            model: root_model,
            config_profile: root_config_profile,
            access_mode: root_access_mode,
            full_auto: root_full_auto,
            dangerously_bypass_approvals: root_dangerously_bypass_approvals,
            cwd: root_cwd,
            add_dir: root_add_dir,
        } = root;

        if model.is_none() {
            model.clone_from(root_model);
        }
        if config_profile.is_none() {
            config_profile.clone_from(root_config_profile);
        }
        if access_mode.is_none() {
            *access_mode = *root_access_mode;
        }
        if !self_selected_access_mode {
            *full_auto = *root_full_auto;
            *dangerously_bypass_approvals = *root_dangerously_bypass_approvals;
        }
        if cwd.is_none() {
            cwd.clone_from(root_cwd);
        }
        if !root_images.is_empty() {
            let mut merged_images = root_images.clone();
            merged_images.append(images);
            *images = merged_images;
        }
        if !root_add_dir.is_empty() {
            let mut merged_add_dir = root_add_dir.clone();
            merged_add_dir.append(add_dir);
            *add_dir = merged_add_dir;
        }
    }

    pub fn apply_subcommand_overrides(&mut self, subcommand: Self) {
        let subcommand_selected_access_mode = subcommand.access_mode.is_some()
            || subcommand.full_auto
            || subcommand.dangerously_bypass_approvals;
        let Self {
            images,
            model,
            config_profile,
            access_mode,
            full_auto,
            dangerously_bypass_approvals,
            cwd,
            add_dir,
        } = subcommand;

        if let Some(model) = model {
            self.model = Some(model);
        }
        if let Some(config_profile) = config_profile {
            self.config_profile = Some(config_profile);
        }
        if subcommand_selected_access_mode {
            self.access_mode = access_mode;
            self.full_auto = full_auto;
            self.dangerously_bypass_approvals = dangerously_bypass_approvals;
        }
        if let Some(cwd) = cwd {
            self.cwd = Some(cwd);
        }
        if !images.is_empty() {
            self.images = images;
        }
        if !add_dir.is_empty() {
            self.add_dir.extend(add_dir);
        }
    }
}
