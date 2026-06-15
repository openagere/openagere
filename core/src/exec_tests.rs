use super::*;
use agere_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

fn make_exec_output(
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    aggregated: &str,
) -> ExecToolCallOutput {
    ExecToolCallOutput {
        exit_code,
        stdout: StreamOutput::new(stdout.to_string()),
        stderr: StreamOutput::new(stderr.to_string()),
        aggregated_output: StreamOutput::new(aggregated.to_string()),
        duration: Duration::from_millis(1),
        timed_out: false,
    }
}

#[test]
fn policy_denial_detection_requires_keywords() {
    let output = make_exec_output(/*exit_code*/ 1, "", "", "");
    assert!(!is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_identifies_keyword_in_stderr() {
    let output = make_exec_output(/*exit_code*/ 1, "", "Operation not permitted", "");
    assert!(is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_identifies_seatbelt_denial_in_stderr() {
    let output = make_exec_output(
        /*exit_code*/ 1,
        "",
        "Seatbelt policy deny: network-outbound",
        "",
    );
    assert!(is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_respects_quick_reject_exit_codes() {
    let output = make_exec_output(/*exit_code*/ 127, "", "command not found", "");
    assert!(!is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_ignores_plain_failure_without_policy_signal() {
    let output = make_exec_output(
        /*exit_code*/ 1,
        "",
        "build failed: wrong rust edition",
        "",
    );
    assert!(!is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_ignores_network_policy_json_without_filesystem_denial() {
    let output = make_exec_output(
        /*exit_code*/ 0,
        "",
        "",
        r#"AGERE_NETWORK_POLICY_DECISION {"decision":"ask","reason":"not_allowed","source":"decider","protocol":"http","host":"google.com","port":80}"#,
    );
    assert!(!is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_uses_aggregated_output() {
    let output = make_exec_output(
        /*exit_code*/ 101,
        "",
        "",
        "cargo failed: Read-only file system when writing target",
    );
    assert!(is_likely_policy_denial(&output));
}

#[test]
fn policy_denial_detection_ignores_network_policy_text_with_zero_exit_code() {
    let output = make_exec_output(
        /*exit_code*/ 0,
        "",
        "",
        r#"AGERE_NETWORK_POLICY_DECISION {"decision":"ask","source":"decider","protocol":"http","host":"google.com","port":80}"#,
    );

    assert!(!is_likely_policy_denial(&output));
}

#[tokio::test]
async fn read_output_limits_retained_bytes_for_shell_capture() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    let bytes = vec![b'a'; EXEC_OUTPUT_MAX_BYTES.saturating_add(128 * 1024)];
    tokio::spawn(async move {
        writer.write_all(&bytes).await.expect("write");
    });

    let out = read_output(
        reader,
        /*stream*/ None,
        /*is_stderr*/ false,
        Some(EXEC_OUTPUT_MAX_BYTES),
    )
    .await
    .expect("read");
    assert_eq!(out.text.len(), EXEC_OUTPUT_MAX_BYTES);
}

#[test]
fn aggregate_output_prefers_stderr_on_contention() {
    let stdout = StreamOutput {
        text: vec![b'a'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: vec![b'b'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };

    let aggregated = aggregate_output(&stdout, &stderr, Some(EXEC_OUTPUT_MAX_BYTES));
    let stdout_cap = EXEC_OUTPUT_MAX_BYTES / 3;
    let stderr_cap = EXEC_OUTPUT_MAX_BYTES.saturating_sub(stdout_cap);

    assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
    assert_eq!(aggregated.text[..stdout_cap], vec![b'a'; stdout_cap]);
    assert_eq!(aggregated.text[stdout_cap..], vec![b'b'; stderr_cap]);
}

#[test]
fn aggregate_output_fills_remaining_capacity_with_stderr() {
    let stdout_len = EXEC_OUTPUT_MAX_BYTES / 10;
    let stdout = StreamOutput {
        text: vec![b'a'; stdout_len],
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: vec![b'b'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };

    let aggregated = aggregate_output(&stdout, &stderr, Some(EXEC_OUTPUT_MAX_BYTES));
    let stderr_cap = EXEC_OUTPUT_MAX_BYTES.saturating_sub(stdout_len);

    assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
    assert_eq!(aggregated.text[..stdout_len], vec![b'a'; stdout_len]);
    assert_eq!(aggregated.text[stdout_len..], vec![b'b'; stderr_cap]);
}

#[test]
fn aggregate_output_rebalances_when_stderr_is_small() {
    let stdout = StreamOutput {
        text: vec![b'a'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: vec![b'b'; 1],
        truncated_after_lines: None,
    };

    let aggregated = aggregate_output(&stdout, &stderr, Some(EXEC_OUTPUT_MAX_BYTES));
    let stdout_len = EXEC_OUTPUT_MAX_BYTES.saturating_sub(1);

    assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
    assert_eq!(aggregated.text[..stdout_len], vec![b'a'; stdout_len]);
    assert_eq!(aggregated.text[stdout_len..], vec![b'b'; 1]);
}

#[test]
fn aggregate_output_keeps_stdout_then_stderr_when_under_cap() {
    let stdout = StreamOutput {
        text: vec![b'a'; 4],
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: vec![b'b'; 3],
        truncated_after_lines: None,
    };

    let aggregated = aggregate_output(&stdout, &stderr, Some(EXEC_OUTPUT_MAX_BYTES));
    let mut expected = Vec::new();
    expected.extend_from_slice(&stdout.text);
    expected.extend_from_slice(&stderr.text);

    assert_eq!(aggregated.text, expected);
    assert_eq!(aggregated.truncated_after_lines, None);
}

#[tokio::test]
async fn read_output_retains_all_bytes_for_full_buffer_capture() {
    let (mut writer, reader) = tokio::io::duplex(1024);
    let bytes = vec![b'a'; EXEC_OUTPUT_MAX_BYTES.saturating_add(128 * 1024)];
    let expected_len = bytes.len();
    // The duplex pipe is smaller than `bytes`, so the writer must run concurrently
    // with `read_output()` or `write_all()` will block once the buffer fills up.
    tokio::spawn(async move {
        writer.write_all(&bytes).await.expect("write");
    });

    let out = read_output(
        reader, /*stream*/ None, /*is_stderr*/ false, /*max_bytes*/ None,
    )
    .await
    .expect("read");
    assert_eq!(out.text.len(), expected_len);
}

#[test]
fn aggregate_output_keeps_all_bytes_when_uncapped() {
    let stdout = StreamOutput {
        text: vec![b'a'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: vec![b'b'; EXEC_OUTPUT_MAX_BYTES],
        truncated_after_lines: None,
    };

    let aggregated = aggregate_output(&stdout, &stderr, /*max_bytes*/ None);

    assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES * 2);
    assert_eq!(
        aggregated.text[..EXEC_OUTPUT_MAX_BYTES],
        vec![b'a'; EXEC_OUTPUT_MAX_BYTES]
    );
    assert_eq!(
        aggregated.text[EXEC_OUTPUT_MAX_BYTES..],
        vec![b'b'; EXEC_OUTPUT_MAX_BYTES]
    );
}

#[test]
fn full_buffer_capture_policy_disables_caps_and_exec_expiration() {
    assert_eq!(ExecCapturePolicy::FullBuffer.retained_bytes_cap(), None);
    assert_eq!(
        ExecCapturePolicy::FullBuffer.io_drain_timeout(),
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS)
    );
    assert!(!ExecCapturePolicy::FullBuffer.uses_expiration());
}

#[tokio::test]
async fn exec_full_buffer_capture_ignores_expiration() -> Result<()> {
    #[cfg(windows)]
    let command = vec![
        "powershell.exe".to_string(),
        "-NonInteractive".to_string(),
        "-NoLogo".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Milliseconds 50; [Console]::Out.Write('hello')".to_string(),
    ];
    #[cfg(not(windows))]
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 0.05; printf hello".to_string(),
    ];

    let env: HashMap<String, String> = std::env::vars().collect();
    let output = exec(
        ExecParams {
            command,
            cwd: agere_utils_fs::AbsolutePathBuf::current_dir()?,
            expiration: 1.into(),
            capture_policy: ExecCapturePolicy::FullBuffer,
            env,
            network: None,
            permission_level: ExecutionPermissionLevel::UseDefault,
            justification: None,
            arg0: None,
        },
        NetworkAccessPolicy::Enabled,
        /*stdout_stream*/ None,
        /*after_spawn*/ None,
    )
    .await?;

    assert_eq!(output.stdout.from_utf8_lossy().text.trim(), "hello");
    assert!(!output.timed_out);

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn exec_full_buffer_capture_keeps_io_drain_timeout_when_descendant_holds_pipe_open()
-> Result<()> {
    let output = tokio::time::timeout(
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS * 3),
        exec(
            ExecParams {
                command: vec![
                    "/bin/sh".to_string(),
                    "-c".to_string(),
                    "printf hello; sleep 30 &".to_string(),
                ],
                cwd: agere_utils_fs::AbsolutePathBuf::current_dir()?,
                expiration: 1.into(),
                capture_policy: ExecCapturePolicy::FullBuffer,
                env: std::env::vars().collect(),
                network: None,
                permission_level: ExecutionPermissionLevel::UseDefault,
                justification: None,
                arg0: None,
            },
            NetworkAccessPolicy::Enabled,
            /*stdout_stream*/ None,
            /*after_spawn*/ None,
        ),
    )
    .await
    .expect("full-buffer exec should return once the I/O drain guard fires")?;

    assert!(!output.timed_out);

    Ok(())
}

#[tokio::test]
async fn process_exec_tool_call_preserves_full_buffer_capture_policy() -> Result<()> {
    let byte_count = EXEC_OUTPUT_MAX_BYTES.saturating_add(128 * 1024);
    #[cfg(windows)]
    let command = vec![
        "powershell.exe".to_string(),
        "-NonInteractive".to_string(),
        "-NoLogo".to_string(),
        "-Command".to_string(),
        format!("Start-Sleep -Milliseconds 50; [Console]::Out.Write('a' * {byte_count})"),
    ];
    #[cfg(not(windows))]
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!("sleep 0.05; head -c {byte_count} /dev/zero | tr '\\0' 'a'"),
    ];

    let cwd = agere_utils_fs::AbsolutePathBuf::current_dir()?;
    let access_policy = "danger-full-access".to_string();
    let permission_profile = PermissionProfile::from_legacy_access_policy(&access_policy);
    let output = process_exec_tool_call(
        ExecParams {
            command,
            cwd: cwd.clone(),
            expiration: 1.into(),
            capture_policy: ExecCapturePolicy::FullBuffer,
            env: std::env::vars().collect(),
            network: None,
            permission_level: ExecutionPermissionLevel::UseDefault,
            justification: None,
            arg0: None,
        },
        &permission_profile,
        &cwd,
        &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await?;

    assert!(!output.timed_out);
    assert_eq!(output.stdout.text.len(), byte_count);

    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn kill_child_process_group_kills_grandchildren_on_timeout() -> Result<()> {
    // On Linux/macOS, /bin/bash is typically present; on FreeBSD/OpenBSD,
    // prefer /bin/sh to avoid NotFound errors.
    #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
    let command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 60 & echo $!; sleep 60".to_string(),
    ];
    #[cfg(all(unix, not(any(target_os = "freebsd", target_os = "openbsd"))))]
    let command = vec![
        "/bin/bash".to_string(),
        "-c".to_string(),
        "sleep 60 & echo $!; sleep 60".to_string(),
    ];
    let cwd = agere_utils_fs::AbsolutePathBuf::current_dir()?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let params = ExecParams {
        command,
        cwd,
        expiration: 500.into(),
        capture_policy: ExecCapturePolicy::ShellTool,
        env,
        network: None,
        permission_level: ExecutionPermissionLevel::UseDefault,
        justification: None,
        arg0: None,
    };

    let output = exec(
        params,
        NetworkAccessPolicy::Restricted,
        /*stdout_stream*/ None,
        /*after_spawn*/ None,
    )
    .await?;
    assert!(output.timed_out);

    let stdout = output.stdout.from_utf8_lossy().text;
    let pid_line = stdout.lines().next().unwrap_or("").trim();
    let pid: i32 = pid_line.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse pid from stdout '{pid_line}': {error}"),
        )
    })?;

    let mut killed = false;
    for _ in 0..20 {
        // Use kill(pid, 0) to check if the process is alive.
        if unsafe { libc::kill(pid, 0) } == -1
            && let Some(libc::ESRCH) = std::io::Error::last_os_error().raw_os_error()
        {
            killed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    assert!(killed, "grandchild process with pid {pid} is still alive");
    Ok(())
}

#[tokio::test]
async fn process_exec_tool_call_respects_cancellation_token() -> Result<()> {
    let command = long_running_command();
    let cwd = agere_utils_fs::AbsolutePathBuf::current_dir()?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let cancel_token = CancellationToken::new();
    let cancel_tx = cancel_token.clone();
    let params = ExecParams {
        command,
        cwd: cwd.clone(),
        expiration: ExecExpiration::Cancellation(cancel_token),
        capture_policy: ExecCapturePolicy::ShellTool,
        env,
        network: None,
        permission_level: ExecutionPermissionLevel::UseDefault,
        justification: None,
        arg0: None,
    };
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1_000)).await;
        cancel_tx.cancel();
    });
    let permission_profile = PermissionProfile::Disabled;
    let result = process_exec_tool_call(
        params,
        &permission_profile,
        &cwd,
        &None,
        /*use_legacy_landlock*/ false,
        /*stdout_stream*/ None,
    )
    .await;
    let output = match result {
        Err(AgereErr::Exec(ExecErr::Timeout { output })) => output,
        other => panic!("expected timeout error, got {other:?}"),
    };
    assert!(output.timed_out);
    assert_eq!(output.exit_code, EXEC_TIMEOUT_EXIT_CODE);
    Ok(())
}

#[cfg(unix)]
fn long_running_command() -> Vec<String> {
    vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 30".to_string(),
    ]
}

#[cfg(windows)]
fn long_running_command() -> Vec<String> {
    vec![
        "powershell.exe".to_string(),
        "-NonInteractive".to_string(),
        "-NoLogo".to_string(),
        "-Command".to_string(),
        "Start-Sleep -Seconds 30".to_string(),
    ]
}
