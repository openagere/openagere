#![allow(clippy::expect_used)]
use agere_login::AGERE_API_KEY_ENV_VAR;
use std::path::Path;
use tempfile::TempDir;
use wiremock::MockServer;

pub struct TestAgereExecBuilder {
    home: TempDir,
    cwd: TempDir,
}

impl TestAgereExecBuilder {
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::new(
            agere_utils_cargo_bin::cargo_bin("agere-exec")
                .expect("should find binary for agere-exec"),
        );
        cmd.current_dir(self.cwd.path())
            .env("AGERE_HOME", self.home.path())
            .env("AGERE_SQLITE_HOME", self.home.path())
            .env(AGERE_API_KEY_ENV_VAR, "dummy");
        cmd
    }
    pub fn cmd_with_server(&self, server: &MockServer) -> assert_cmd::Command {
        let mut cmd = self.cmd();
        let base = format!("{}/v1", server.uri());
        cmd.arg("-c")
            .arg(format!("openai_base_url={}", toml_string_literal(&base)));
        cmd
    }

    pub fn cwd_path(&self) -> &Path {
        self.cwd.path()
    }
    pub fn home_path(&self) -> &Path {
        self.home.path()
    }
}

fn toml_string_literal(value: &str) -> String {
    serde_json::to_string(value).expect("serialize TOML string literal")
}

pub fn test_agere_exec() -> TestAgereExecBuilder {
    TestAgereExecBuilder {
        home: TempDir::new().expect("create temp home"),
        cwd: TempDir::new().expect("create temp cwd"),
    }
}
