//! Terminal detection utilities.
//!
//! Inlined from the former `agere-terminal-detection` crate.

use std::sync::OnceLock;

/// Structured terminal identification data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalInfo {
    /// The detected terminal name category.
    pub(crate) name: TerminalName,
    /// The `TERM_PROGRAM` value when provided by the terminal.
    pub(crate) term_program: Option<String>,
    /// The terminal version string when available.
    pub(crate) version: Option<String>,
    /// The `TERM` value when falling back to capability strings.
    pub(crate) term: Option<String>,
    /// Multiplexer metadata when a terminal multiplexer is active.
    pub(crate) multiplexer: Option<Multiplexer>,
}

/// Known terminal name categories derived from environment variables.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalName {
    /// Apple Terminal (Terminal.app).
    AppleTerminal,
    /// Ghostty terminal emulator.
    Ghostty,
    /// iTerm2 terminal emulator.
    Iterm2,
    /// Warp terminal emulator.
    WarpTerminal,
    /// Visual Studio Code integrated terminal.
    VsCode,
    /// WezTerm terminal emulator.
    WezTerm,
    /// kitty terminal emulator.
    Kitty,
    /// Alacritty terminal emulator.
    Alacritty,
    /// KDE Konsole terminal emulator.
    Konsole,
    /// GNOME Terminal emulator.
    GnomeTerminal,
    /// VTE backend terminal.
    Vte,
    /// Windows Terminal emulator.
    WindowsTerminal,
    /// Dumb terminal (TERM=dumb).
    Dumb,
    /// Unknown or missing terminal identification.
    Unknown,
}

/// Detected terminal multiplexer metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Multiplexer {
    /// tmux terminal multiplexer.
    Tmux {
        /// tmux version string when `TERM_PROGRAM=tmux` is available.
        version: Option<String>,
    },
    /// zellij terminal multiplexer.
    Zellij {},
}

/// tmux client terminal identification captured via `tmux display-message`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TmuxClientInfo {
    termtype: Option<String>,
    termname: Option<String>,
}

impl TerminalInfo {
    fn new(
        name: TerminalName,
        term_program: Option<String>,
        version: Option<String>,
        term: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self {
            name,
            term_program,
            version,
            term,
            multiplexer,
        }
    }

    fn from_term_program(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(
            name,
            Some(term_program),
            version,
            /*term*/ None,
            multiplexer,
        )
    }

    fn from_term_program_and_term(
        name: TerminalName,
        term_program: String,
        version: Option<String>,
        term: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(name, Some(term_program), version, term, multiplexer)
    }

    fn from_name(
        name: TerminalName,
        version: Option<String>,
        multiplexer: Option<Multiplexer>,
    ) -> Self {
        Self::new(
            name,
            /*term_program*/ None,
            version,
            /*term*/ None,
            multiplexer,
        )
    }

    fn from_term(term: String, multiplexer: Option<Multiplexer>) -> Self {
        let name = match term.as_str() {
            "dumb" => TerminalName::Dumb,
            "wezterm" | "wezterm-mux" => TerminalName::WezTerm,
            _ => TerminalName::Unknown,
        };
        Self::new(
            name,
            /*term_program*/ None,
            /*version*/ None,
            Some(term),
            multiplexer,
        )
    }

    fn unknown(multiplexer: Option<Multiplexer>) -> Self {
        Self::new(
            TerminalName::Unknown,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            multiplexer,
        )
    }

    pub(crate) fn user_agent_token(&self) -> String {
        let raw = if let Some(program) = self.term_program.as_ref() {
            match self.version.as_ref().filter(|v| !v.is_empty()) {
                Some(version) => format!("{program}/{version}"),
                None => program.clone(),
            }
        } else if let Some(term) = self.term.as_ref().filter(|value| !value.is_empty()) {
            term.clone()
        } else {
            match self.name {
                TerminalName::AppleTerminal => {
                    format_terminal_version("Apple_Terminal", &self.version)
                }
                TerminalName::Ghostty => format_terminal_version("Ghostty", &self.version),
                TerminalName::Iterm2 => format_terminal_version("iTerm.app", &self.version),
                TerminalName::WarpTerminal => {
                    format_terminal_version("WarpTerminal", &self.version)
                }
                TerminalName::VsCode => format_terminal_version("vscode", &self.version),
                TerminalName::WezTerm => format_terminal_version("WezTerm", &self.version),
                TerminalName::Kitty => "kitty".to_string(),
                TerminalName::Alacritty => "Alacritty".to_string(),
                TerminalName::Konsole => format_terminal_version("Konsole", &self.version),
                TerminalName::GnomeTerminal => "gnome-terminal".to_string(),
                TerminalName::Vte => format_terminal_version("VTE", &self.version),
                TerminalName::WindowsTerminal => "WindowsTerminal".to_string(),
                TerminalName::Dumb => "dumb".to_string(),
                TerminalName::Unknown => "unknown".to_string(),
            }
        };

        sanitize_header_value(raw)
    }

    /// Returns whether the active terminal multiplexer is Zellij.
    #[allow(dead_code)]
    pub(crate) fn is_zellij(&self) -> bool {
        matches!(self.multiplexer, Some(Multiplexer::Zellij {}))
    }
}

static TERMINAL_INFO: OnceLock<TerminalInfo> = OnceLock::new();

trait Environment {
    fn var(&self, name: &str) -> Option<String>;

    fn has(&self, name: &str) -> bool {
        self.var(name).is_some()
    }

    fn var_non_empty(&self, name: &str) -> Option<String> {
        self.var(name).and_then(none_if_whitespace)
    }

    fn has_non_empty(&self, name: &str) -> bool {
        self.var_non_empty(name).is_some()
    }

    fn tmux_client_info(&self) -> TmuxClientInfo;
}

struct ProcessEnvironment;

impl Environment for ProcessEnvironment {
    fn var(&self, name: &str) -> Option<String> {
        match std::env::var(name) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                tracing::warn!("failed to read env var {name}: value not valid UTF-8");
                None
            }
        }
    }

    fn tmux_client_info(&self) -> TmuxClientInfo {
        tmux_client_info()
    }
}

/// Returns a sanitized terminal identifier for User-Agent strings.
pub(crate) fn user_agent() -> String {
    terminal_info().user_agent_token()
}

/// Returns structured terminal metadata for the current process.
pub(crate) fn terminal_info() -> TerminalInfo {
    TERMINAL_INFO
        .get_or_init(|| detect_terminal_info_from_env(&ProcessEnvironment))
        .clone()
}

fn detect_terminal_info_from_env(env: &dyn Environment) -> TerminalInfo {
    let multiplexer = detect_multiplexer(env);

    if let Some(term_program) = env.var_non_empty("TERM_PROGRAM") {
        if is_tmux_term_program(&term_program)
            && matches!(multiplexer, Some(Multiplexer::Tmux { .. }))
            && let Some(terminal) =
                terminal_from_tmux_client_info(env.tmux_client_info(), multiplexer.clone())
        {
            return terminal;
        }

        let version = env.var_non_empty("TERM_PROGRAM_VERSION");
        let name = terminal_name_from_term_program(&term_program).unwrap_or(TerminalName::Unknown);
        return TerminalInfo::from_term_program(name, term_program, version, multiplexer);
    }

    if env.has("WEZTERM_VERSION") {
        let version = env.var_non_empty("WEZTERM_VERSION");
        return TerminalInfo::from_name(TerminalName::WezTerm, version, multiplexer);
    }

    if env.has("ITERM_SESSION_ID") || env.has("ITERM_PROFILE") || env.has("ITERM_PROFILE_NAME") {
        return TerminalInfo::from_name(TerminalName::Iterm2, /*version*/ None, multiplexer);
    }

    if env.has("TERM_SESSION_ID") {
        return TerminalInfo::from_name(
            TerminalName::AppleTerminal,
            /*version*/ None,
            multiplexer,
        );
    }

    if env.has("KITTY_WINDOW_ID")
        || env
            .var("TERM")
            .map(|term| term.contains("kitty"))
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(TerminalName::Kitty, /*version*/ None, multiplexer);
    }

    if env.has("ALACRITTY_SOCKET")
        || env
            .var("TERM")
            .map(|term| term == "alacritty")
            .unwrap_or(false)
    {
        return TerminalInfo::from_name(
            TerminalName::Alacritty,
            /*version*/ None,
            multiplexer,
        );
    }

    if env.has("KONSOLE_VERSION") {
        let version = env.var_non_empty("KONSOLE_VERSION");
        return TerminalInfo::from_name(TerminalName::Konsole, version, multiplexer);
    }

    if env.has("GNOME_TERMINAL_SCREEN") {
        return TerminalInfo::from_name(
            TerminalName::GnomeTerminal,
            /*version*/ None,
            multiplexer,
        );
    }

    if env.has("VTE_VERSION") {
        let version = env.var_non_empty("VTE_VERSION");
        return TerminalInfo::from_name(TerminalName::Vte, version, multiplexer);
    }

    if env.has("WT_SESSION") {
        return TerminalInfo::from_name(
            TerminalName::WindowsTerminal,
            /*version*/ None,
            multiplexer,
        );
    }

    if let Some(term) = env.var_non_empty("TERM") {
        return TerminalInfo::from_term(term, multiplexer);
    }

    TerminalInfo::unknown(multiplexer)
}

fn detect_multiplexer(env: &dyn Environment) -> Option<Multiplexer> {
    if env.has_non_empty("TMUX") || env.has_non_empty("TMUX_PANE") {
        return Some(Multiplexer::Tmux {
            version: tmux_version_from_env(env),
        });
    }

    if env.has_non_empty("ZELLIJ")
        || env.has_non_empty("ZELLIJ_SESSION_NAME")
        || env.has_non_empty("ZELLIJ_VERSION")
    {
        return Some(Multiplexer::Zellij {});
    }

    None
}

fn is_tmux_term_program(value: &str) -> bool {
    value.eq_ignore_ascii_case("tmux")
}

fn terminal_from_tmux_client_info(
    client_info: TmuxClientInfo,
    multiplexer: Option<Multiplexer>,
) -> Option<TerminalInfo> {
    let termtype = client_info.termtype.and_then(none_if_whitespace);
    let termname = client_info.termname.and_then(none_if_whitespace);

    if let Some(termtype) = termtype.as_ref() {
        let (program, version) = split_term_program_and_version(termtype);
        let name = terminal_name_from_term_program(&program).unwrap_or(TerminalName::Unknown);
        return Some(TerminalInfo::from_term_program_and_term(
            name,
            program,
            version,
            termname,
            multiplexer,
        ));
    }

    termname
        .as_ref()
        .map(|termname| TerminalInfo::from_term(termname.to_string(), multiplexer))
}

fn tmux_version_from_env(env: &dyn Environment) -> Option<String> {
    let term_program = env.var("TERM_PROGRAM")?;
    if !is_tmux_term_program(&term_program) {
        return None;
    }

    env.var_non_empty("TERM_PROGRAM_VERSION")
}

fn split_term_program_and_version(value: &str) -> (String, Option<String>) {
    let mut parts = value.split_whitespace();
    let program = parts.next().unwrap_or_default().to_string();
    let version = parts.next().map(ToString::to_string);
    (program, version)
}

fn tmux_client_info() -> TmuxClientInfo {
    let termtype = tmux_display_message("#{client_termtype}");
    let termname = tmux_display_message("#{client_termname}");

    TmuxClientInfo { termtype, termname }
}

fn tmux_display_message(format: &str) -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", format])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    none_if_whitespace(value.trim().to_string())
}

fn sanitize_header_value(value: String) -> String {
    value.replace(|c| !is_valid_header_value_char(c), "_")
}

fn is_valid_header_value_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
}

fn terminal_name_from_term_program(value: &str) -> Option<TerminalName> {
    let normalized: String = value
        .trim()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_' | '.'))
        .map(|c| c.to_ascii_lowercase())
        .collect();

    match normalized.as_str() {
        "appleterminal" => Some(TerminalName::AppleTerminal),
        "ghostty" => Some(TerminalName::Ghostty),
        "iterm" | "iterm2" | "itermapp" => Some(TerminalName::Iterm2),
        "warp" | "warpterminal" => Some(TerminalName::WarpTerminal),
        "vscode" => Some(TerminalName::VsCode),
        "wezterm" => Some(TerminalName::WezTerm),
        "kitty" => Some(TerminalName::Kitty),
        "alacritty" => Some(TerminalName::Alacritty),
        "konsole" => Some(TerminalName::Konsole),
        "gnometerminal" => Some(TerminalName::GnomeTerminal),
        "vte" => Some(TerminalName::Vte),
        "windowsterminal" => Some(TerminalName::WindowsTerminal),
        "dumb" => Some(TerminalName::Dumb),
        _ => None,
    }
}

fn format_terminal_version(name: &str, version: &Option<String>) -> String {
    match version.as_ref().filter(|value| !value.is_empty()) {
        Some(version) => format!("{name}/{version}"),
        None => name.to_string(),
    }
}

fn none_if_whitespace(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
