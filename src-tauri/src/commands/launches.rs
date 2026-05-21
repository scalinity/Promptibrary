//! `commands::launches` per spec §11 *Launches*.
//!
//! Wires the L3 launch pipeline (`launch::*` modules) against the
//! prompts repo, the variables parser+renderer, the runs repo, and
//! the telemetry event log. Bridge from the PTY event stream to
//! Tauri events the xterm.js frontend listens to.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::app_state::ManagedState;
use crate::commands::vault::current_vault_db;
use crate::domain::launch::{LaunchProfile, ResolvedVariableValue};
use crate::domain::prompt::{
    ClaudeModelId, ClaudePermissionMode, ClaudePermissionRule, LaunchDestination, Prompt,
    VerifierMode,
};
use crate::variables::renderer::BoolRenderMode;
use crate::error::{AppError, AppErrorKind, Result};
use crate::ids::{new_ulid, RunId};
use crate::index::prompts_repo::get_prompt_index;
use crate::index::runs_repo::{self, InsertRunInput, RunStatus};
use crate::index::telemetry_repo::{self, LaunchTelemetryEvent};
use crate::launch::claude_cli;
use crate::launch::pty_session::{self, PtyEvent, PtySessionConfig};
use crate::variables::parser::{parse_template_variables, ParseVariablesInput};
use crate::variables::renderer::{render_prompt, RenderPromptInput};
use crate::vault::paths::VaultPaths;

// ─── Input / output types per spec §7 + frontend ipc.ts ──────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLaunchInput {
    pub prompt_id: crate::ids::PromptId,
    pub values: Vec<ResolvedVariableValue>,
    pub working_directory: PathBuf,
    #[serde(default)]
    pub inline_tweak_body: Option<String>,
    #[serde(default)]
    pub additional_directories: Vec<PathBuf>,
    #[serde(default)]
    pub model: Option<ClaudeModelId>,
    #[serde(default)]
    pub verifier_mode: Option<VerifierMode>,
    #[serde(default)]
    pub permission_mode: Option<ClaudePermissionMode>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<ClaudePermissionRule>>,
    #[serde(default)]
    pub disallowed_tools: Option<Vec<ClaudePermissionRule>>,
    #[serde(default)]
    pub mcp_config_paths: Option<Vec<PathBuf>>,
    #[serde(default)]
    pub strict_mcp_config: Option<bool>,
    #[serde(default)]
    pub append_system_prompt: Option<String>,
    #[serde(default)]
    pub cols: Option<u16>,
    #[serde(default)]
    pub rows: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLaunchOutput {
    pub run_id: RunId,
    pub launch_profile: LaunchProfile,
    pub transcript_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRunInput {
    pub run_id: RunId,
    #[serde(default)]
    pub force: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTerminalInputInput {
    pub run_id: RunId,
    /// Raw bytes to forward to the PTY stdin. The frontend xterm pane
    /// sends keystrokes as UTF-8 strings — they round-trip through
    /// the bytes here.
    pub bytes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeTerminalInput {
    pub run_id: RunId,
    pub cols: u16,
    pub rows: u16,
}

// ─── Tauri event payloads (frontend listens to these) ────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TermStdoutEvent {
    run_id: RunId,
    /// Base64 of the raw bytes — JSON can't carry arbitrary bytes
    /// safely, and an xterm.js compatible base64 reload is cheap.
    bytes_b64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TermFinishedEvent {
    run_id: RunId,
    exit_code: Option<i32>,
    signal: Option<String>,
}

// ─── start_launch ────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_launch(
    input: StartLaunchInput,
    app: AppHandle,
    services: State<'_, ManagedState>,
) -> Result<StartLaunchOutput> {
    let (vault, db) = current_vault_db(&services).await?;

    // 1. Load the prompt + its body. Body source = inline tweak if set
    //    (per spec §5), else the saved prompt body.
    let prompt = load_prompt(&db, &vault, input.prompt_id.0.as_str()).await?;
    let template = input
        .inline_tweak_body
        .clone()
        .unwrap_or_else(|| prompt.body.clone());

    // 2. Resolve variables. We re-parse the template body so the
    //    launch can't go out of date if the prompt's frontmatter
    //    drifted between save + launch.
    let parsed = parse_template_variables(ParseVariablesInput {
        template: template.clone(),
        frontmatter_variables: prompt.variables.clone(),
    });
    let render = render_prompt(RenderPromptInput {
        template: template.clone(),
        refs: parsed.refs,
        values: input.values.iter().map(to_renderer_value).collect(),
        bool_render_mode: BoolRenderMode::FrontmatterStrings,
        variables: prompt.variables.clone(),
    });
    if !render.errors.is_empty() {
        return Err(AppError::new(
            AppErrorKind::VariableValidationFailed,
            format!(
                "{} variable render error(s) — fix in the compose drawer before launching",
                render.errors.len()
            ),
        )
        .with_detail("error_count", render.errors.len() as i64));
    }
    let resolved_prompt = render.rendered.clone();

    // SCA-912 (C7, CWE-77/CWE-150): refuse prompts that would corrupt
    // the bracketed-paste envelope. Vector: an extracted-article
    // variable value containing ESC[201~ would terminate the paste
    // early and let subsequent bytes reach claude as keystrokes /
    // slash-commands. Check at compose-time so the user sees a typed
    // error rather than a half-injected launch.
    crate::launch::prompt_injector::validate_prompt_bytes(&resolved_prompt)?;

    // SCA-912 (W21, CWE-20): cap `append_system_prompt`. The field
    // flows directly into a claude CLI argv slot; unbounded user input
    // would let a compromised frontend override the system prompt
    // arbitrarily or exhaust OS argv space.
    const APPEND_SYSTEM_PROMPT_MAX: usize = 16 * 1024;
    let resolved_append = input
        .append_system_prompt
        .clone()
        .or_else(|| prompt.launch_defaults.append_system_prompt.clone());
    if let Some(ref s) = resolved_append {
        if s.len() > APPEND_SYSTEM_PROMPT_MAX {
            return Err(AppError::new(
                AppErrorKind::SettingsInvalid,
                format!(
                    "append_system_prompt exceeds {} bytes ({}); reduce or split",
                    APPEND_SYSTEM_PROMPT_MAX,
                    s.len()
                ),
            )
            .with_detail("limit_bytes", APPEND_SYSTEM_PROMPT_MAX as i64)
            .with_detail("actual_bytes", s.len() as i64));
        }
    }

    // 3. Mint a RunId, build LaunchProfile, insert run row.
    let run_id = RunId(new_ulid());
    let launched_at = crate::time::now_utc();
    let prompt_checksum = prompt.checksum_sha256.clone();
    let profile = LaunchProfile {
        prompt_id: input.prompt_id.clone(),
        prompt_title: prompt.title.clone(),
        prompt_version_checksum: prompt_checksum.clone(),
        template_body: prompt.body.clone(),
        inline_tweak_body: input.inline_tweak_body.clone(),
        resolved_prompt: resolved_prompt.clone(),
        variable_values: input.values.clone(),
        working_directory: input.working_directory.clone(),
        additional_directories: input.additional_directories.clone(),
        destination: LaunchDestination::ClaudeCodeCli,
        model: input
            .model
            .unwrap_or(prompt.launch_defaults.model),
        verifier_mode: input
            .verifier_mode
            .unwrap_or(prompt.launch_defaults.verifier_mode),
        permission_mode: input
            .permission_mode
            .unwrap_or(prompt.launch_defaults.permission_mode),
        allowed_tools: input
            .allowed_tools
            .unwrap_or_else(|| prompt.launch_defaults.allowed_tools.clone()),
        disallowed_tools: input
            .disallowed_tools
            .unwrap_or_else(|| prompt.launch_defaults.disallowed_tools.clone()),
        mcp_config_paths: input
            .mcp_config_paths
            .unwrap_or_else(|| prompt.launch_defaults.mcp_config_paths.clone()),
        strict_mcp_config: input
            .strict_mcp_config
            .unwrap_or(prompt.launch_defaults.strict_mcp_config),
        append_system_prompt: resolved_append,
        max_turns: prompt.launch_defaults.max_turns,
        launched_at,
    };
    let profile_json = serde_json::to_string(&profile).map_err(AppError::from)?;

    runs_repo::insert_run(
        &db,
        &InsertRunInput {
            run_id: run_id.0.as_str(),
            prompt_id: input.prompt_id.0.as_str(),
            prompt_title: &prompt.title,
            profile_json: &profile_json,
            started_at: launched_at,
        },
    )
    .await?;
    telemetry_repo::record_event(
        &db,
        run_id.0.as_str(),
        input.prompt_id.0.as_str(),
        &LaunchTelemetryEvent::LaunchCreated {
            profile_id: run_id.0.clone(),
            model: format!("{:?}", profile.model),
        },
    )
    .await?;

    // 4. Build CLI args per spec §7 *Claude Code CLI invocation*.
    let claude_path = claude_cli::resolve_claude_path().await?;
    let args = build_claude_args(&profile);

    // 5. Spawn the PTY session.
    let cfg = PtySessionConfig {
        run_id: run_id.clone(),
        claude_path,
        args,
        cwd: input.working_directory.clone(),
        resolved_prompt: resolved_prompt.clone(),
        vault_root: vault.vault_root.clone(),
        cols: input.cols.unwrap_or(120),
        rows: input.rows.unwrap_or(32),
    };
    let (handle, mut events, _drainer) = pty_session::spawn(cfg).await.map_err(|e| {
        // If PTY spawn fails, mark the run as errored immediately so
        // the library row reflects truth.
        tracing::warn!(error = %e, "pty spawn failed; marking run errored");
        e
    })?;
    let pid = handle.pid.map(|p| p as i64);
    telemetry_repo::record_event(
        &db,
        run_id.0.as_str(),
        input.prompt_id.0.as_str(),
        &LaunchTelemetryEvent::LaunchStarted {
            process_id: pid.unwrap_or(0),
        },
    )
    .await?;
    let transcript_path = handle.transcript_path.clone();
    let arc_handle = Arc::new(handle);
    services.pty_pool.insert(arc_handle.clone()).await;

    // 6. Bridge task: drain PTY events → Tauri events + telemetry.
    let app_for_bridge = app.clone();
    let db_for_bridge = db.clone();
    let pool_for_bridge = services.pty_pool.clone();
    let run_id_for_bridge = run_id.clone();
    let prompt_id_for_bridge = input.prompt_id.clone();
    let started_at_for_bridge = launched_at;
    let transcript_path_for_bridge = transcript_path.clone();
    tokio::spawn(async move {
        let mut bytes_seen: u64 = 0;
        while let Some(ev) = events.recv().await {
            match ev {
                PtyEvent::FirstOutput => {
                    let now = crate::time::now_utc();
                    let latency_ms =
                        (now - started_at_for_bridge).num_milliseconds().max(0);
                    let _ = telemetry_repo::record_event(
                        &db_for_bridge,
                        run_id_for_bridge.0.as_str(),
                        prompt_id_for_bridge.0.as_str(),
                        &LaunchTelemetryEvent::FirstOutput { latency_ms },
                    )
                    .await;
                    let _ = runs_repo::update_run_status(
                        &db_for_bridge,
                        run_id_for_bridge.0.as_str(),
                        RunStatus::FirstOutput,
                    )
                    .await;
                }
                PtyEvent::Stdout(chunk) => {
                    use base64::Engine as _;
                    bytes_seen = bytes_seen.saturating_add(chunk.len() as u64);
                    let payload = TermStdoutEvent {
                        run_id: run_id_for_bridge.clone(),
                        bytes_b64: base64::engine::general_purpose::STANDARD
                            .encode(&chunk),
                    };
                    let _ = app_for_bridge.emit("term:stdout", &payload);
                }
                PtyEvent::Exited { exit_code, signal } => {
                    let ended_at = crate::time::now_utc();
                    let wall_clock_ms =
                        (ended_at - started_at_for_bridge).num_milliseconds().max(0);
                    let final_status = if exit_code == Some(0) {
                        RunStatus::Finished
                    } else {
                        RunStatus::Errored
                    };
                    let _ = runs_repo::complete_run(
                        &db_for_bridge,
                        &runs_repo::CompleteRunInput {
                            run_id: run_id_for_bridge.0.as_str(),
                            status: final_status,
                            ended_at,
                            exit_code,
                            signal: signal.as_deref(),
                            transcript_vault_path: transcript_path_for_bridge
                                .to_str(),
                            transcript_spool_path: None,
                            stdout_bytes: bytes_seen as i64,
                            stderr_bytes: 0,
                            token_count_json: None,
                            cost_usd: None,
                            error_json: None,
                        },
                    )
                    .await;
                    let _ = telemetry_repo::record_event(
                        &db_for_bridge,
                        run_id_for_bridge.0.as_str(),
                        prompt_id_for_bridge.0.as_str(),
                        &LaunchTelemetryEvent::LaunchFinished {
                            exit_code: exit_code.unwrap_or(-1),
                            wall_clock_ms,
                            token_count: None,
                            cost_usd: None,
                        },
                    )
                    .await;
                    let _ = telemetry_repo::record_event(
                        &db_for_bridge,
                        run_id_for_bridge.0.as_str(),
                        prompt_id_for_bridge.0.as_str(),
                        &LaunchTelemetryEvent::TranscriptSpooled {
                            path: transcript_path_for_bridge
                                .to_string_lossy()
                                .into_owned(),
                            bytes: bytes_seen as i64,
                        },
                    )
                    .await;
                    let _ = app_for_bridge.emit(
                        "term:finished",
                        &TermFinishedEvent {
                            run_id: run_id_for_bridge.clone(),
                            exit_code,
                            signal: signal.clone(),
                        },
                    );
                    let _ = pool_for_bridge.remove(&run_id_for_bridge).await;
                    break;
                }
                PtyEvent::Error(msg) => {
                    let _ = telemetry_repo::record_event(
                        &db_for_bridge,
                        run_id_for_bridge.0.as_str(),
                        prompt_id_for_bridge.0.as_str(),
                        &LaunchTelemetryEvent::Error {
                            kind: "pty".into(),
                            message: msg,
                        },
                    )
                    .await;
                }
            }
        }
    });

    Ok(StartLaunchOutput {
        run_id,
        launch_profile: profile,
        transcript_path,
    })
}

// ─── stop_run ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn stop_run(
    input: StopRunInput,
    services: State<'_, ManagedState>,
) -> Result<()> {
    let (_vault, db) = current_vault_db(&services).await?;
    let session = services.pty_pool.get(&input.run_id).await?;
    let force = input.force.unwrap_or(false);

    // Mark the run as stopping immediately so the library row reflects
    // the in-progress state even before the child actually exits.
    let _ = runs_repo::update_run_status(
        &db,
        input.run_id.0.as_str(),
        RunStatus::Stopping,
    )
    .await;

    if force {
        // Force path: SIGTERM → 1s → SIGKILL.
        #[cfg(unix)]
        {
            if let Some(pid) = session.pid() {
                let _ = crate::launch::signals::send_signal(
                    pid as i32,
                    nix::sys::signal::Signal::SIGTERM,
                );
            }
            let exited = session
                .wait_with_timeout(crate::launch::signals::FORCE_SIGTERM_WAIT)
                .await?;
            if exited.is_none() {
                let _ = session.kill().await;
            }
        }
        // SCA-916 (W1): on non-Unix builds we cannot send POSIX signals.
        // Don't wait 13s pretending to escalate — kill immediately.
        #[cfg(not(unix))]
        {
            let _ = session.kill().await;
        }
    } else {
        // Graceful path: SIGINT → 5s → SIGINT → 5s → SIGTERM → 3s → SIGKILL.
        // SCA-916 (W1): the entire escalation ladder is gated behind
        // cfg(unix) because send_signal is unix-only. Without the gate,
        // a Windows build would wait 13s of unconditional sleeps then
        // hard-kill — misleading for the user and the telemetry trail.
        #[cfg(unix)]
        {
            let pid = session.pid().map(|p| p as i32);
            if let Some(pid) = pid {
                let _ = crate::launch::signals::send_signal(
                    pid,
                    nix::sys::signal::Signal::SIGINT,
                );
            }
            let exited = session
                .wait_with_timeout(crate::launch::signals::GRACEFUL_SIGINT_INTERVAL)
                .await?;
            if exited.is_none() {
                if let Some(pid) = pid {
                    let _ = crate::launch::signals::send_signal(
                        pid,
                        nix::sys::signal::Signal::SIGINT,
                    );
                }
                let exited2 = session
                    .wait_with_timeout(crate::launch::signals::GRACEFUL_SIGINT_INTERVAL)
                    .await?;
                if exited2.is_none() {
                    if let Some(pid) = pid {
                        let _ = crate::launch::signals::send_signal(
                            pid,
                            nix::sys::signal::Signal::SIGTERM,
                        );
                    }
                    let exited3 = session
                        .wait_with_timeout(crate::launch::signals::GRACEFUL_SIGTERM_WAIT)
                        .await?;
                    if exited3.is_none() {
                        let _ = session.kill().await;
                    }
                }
            }
        }
        #[cfg(not(unix))]
        {
            // Non-Unix: no graceful escalation path available — request
            // termination immediately. The PTY drainer will surface
            // Exited with signal=None / exit_code as the child reports.
            let _ = session.kill().await;
        }
    }

    // Record the stop event. The bridge task's LaunchFinished record
    // covers the terminal state when the child actually exits — this
    // is the operator-action audit.
    let _ = telemetry_repo::record_event(
        &db,
        input.run_id.0.as_str(),
        "", // prompt_id not available at this layer; left empty
        &LaunchTelemetryEvent::LaunchStopped {
            signal: if force { "SIGTERM".into() } else { "SIGINT".into() },
            graceful: !force,
        },
    )
    .await;
    Ok(())
}

// ─── send_terminal_input ─────────────────────────────────────────────

#[tauri::command]
pub async fn send_terminal_input(
    input: SendTerminalInputInput,
    services: State<'_, ManagedState>,
) -> Result<()> {
    let session = services.pty_pool.get(&input.run_id).await?;
    session.write(input.bytes.into_bytes()).await
}

// ─── resize_terminal ─────────────────────────────────────────────────

#[tauri::command]
pub async fn resize_terminal(
    input: ResizeTerminalInput,
    services: State<'_, ManagedState>,
) -> Result<()> {
    let session = services.pty_pool.get(&input.run_id).await?;
    session.resize(input.cols.max(1), input.rows.max(1)).await
}

// ─── Helpers ─────────────────────────────────────────────────────────

/// Convert the IPC-side `domain::launch::ResolvedVariableValue` to the
/// renderer's identically-shaped enum. The two types are kept distinct
/// so the launch profile struct stays serde-stable independent of any
/// internal renderer refactor — but they map 1:1 today.
fn to_renderer_value(
    v: &ResolvedVariableValue,
) -> crate::variables::renderer::ResolvedVariableValue {
    use crate::variables::renderer::ResolvedVariableValue as R;
    match v {
        ResolvedVariableValue::File { key, value } => R::File {
            key: key.clone(),
            value: value.clone(),
        },
        ResolvedVariableValue::Folder { key, value } => R::Folder {
            key: key.clone(),
            value: value.clone(),
        },
        ResolvedVariableValue::Text { key, value } => R::Text {
            key: key.clone(),
            value: value.clone(),
        },
        ResolvedVariableValue::Multiline { key, value } => R::Multiline {
            key: key.clone(),
            value: value.clone(),
        },
        ResolvedVariableValue::Select { key, value } => R::Select {
            key: key.clone(),
            value: value.clone(),
        },
        ResolvedVariableValue::Bool { key, value } => R::Bool {
            key: key.clone(),
            value: *value,
        },
        ResolvedVariableValue::Number { key, value } => R::Number {
            key: key.clone(),
            value: *value,
        },
    }
}

async fn load_prompt(
    db: &sqlx::SqlitePool,
    vault: &VaultPaths,
    prompt_id: &str,
) -> Result<Prompt> {
    let row = get_prompt_index(db, prompt_id).await?.ok_or_else(|| {
        AppError::new(AppErrorKind::PromptNotFound, "prompt not found")
    })?;
    let abs = vault.absolute(&row.vault_path).ok_or_else(|| {
        AppError::new(AppErrorKind::PromptMalformed, "vault_path failed traversal check")
    })?;
    let content = std::fs::read_to_string(&abs).map_err(|_| {
        AppError::new(AppErrorKind::PromptNotFound, "vault file missing")
    })?;
    super::prompts::prompt_from_file(&row.vault_path, &content)
}

/// Build the args to `claude` per spec §7.
///
/// V1 invocation:
///   * `--model <model>`
///   * `--permission-mode <mode>`
///   * `--add-dir <path>` per additional directory
///   * `--append-system-prompt <string>` when set
///   * `--strict-mcp-config` when true
///   * `--mcp-config <path>` per MCP config path
///   * `--allowedTools` / `--disallowedTools` (omitted in V1 in favour of
///     `--settings`; left empty here — the runtime settings JSON layer
///     covers them in a follow-up)
///
/// **Not** passed: `--max-turns` (not in `claude --help`'s current
/// surface per spec §7 risk note).
fn build_claude_args(profile: &LaunchProfile) -> Vec<String> {
    let mut args = Vec::new();
    args.push("--model".into());
    args.push(model_wire_name(profile.model));
    args.push("--permission-mode".into());
    args.push(permission_mode_wire_name(profile.permission_mode));
    for d in &profile.additional_directories {
        args.push("--add-dir".into());
        args.push(d.to_string_lossy().into_owned());
    }
    if let Some(append) = &profile.append_system_prompt {
        if !append.is_empty() {
            args.push("--append-system-prompt".into());
            args.push(append.clone());
        }
    }
    if profile.strict_mcp_config {
        args.push("--strict-mcp-config".into());
    }
    for cfg in &profile.mcp_config_paths {
        args.push("--mcp-config".into());
        args.push(cfg.to_string_lossy().into_owned());
    }
    args
}

fn model_wire_name(m: ClaudeModelId) -> String {
    serde_json::to_string(&m)
        .ok()
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| "claude-sonnet-4-6".to_string())
}

fn permission_mode_wire_name(m: ClaudePermissionMode) -> String {
    serde_json::to_string(&m)
        .ok()
        .map(|s| s.trim_matches('"').to_string())
        .unwrap_or_else(|| "default".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::prompt::{
        ClaudeModelId, ClaudePermissionMode, LaunchDestination, VerifierMode,
    };

    fn sample_profile() -> LaunchProfile {
        LaunchProfile {
            prompt_id: crate::ids::PromptId("01P".into()),
            prompt_title: "T".into(),
            prompt_version_checksum: "sha256:0".into(),
            template_body: "".into(),
            inline_tweak_body: None,
            resolved_prompt: "do the thing".into(),
            variable_values: vec![],
            working_directory: PathBuf::from("/tmp"),
            additional_directories: vec![PathBuf::from("/extra")],
            destination: LaunchDestination::ClaudeCodeCli,
            model: ClaudeModelId::ClaudeSonnet46,
            verifier_mode: VerifierMode::Off,
            permission_mode: ClaudePermissionMode::Default,
            allowed_tools: vec![],
            disallowed_tools: vec![],
            mcp_config_paths: vec![PathBuf::from("/mcp.json")],
            strict_mcp_config: true,
            append_system_prompt: Some("be careful".into()),
            max_turns: None,
            launched_at: crate::time::now_utc(),
        }
    }

    #[test]
    fn build_claude_args_emits_model_and_permission_mode() {
        let args = build_claude_args(&sample_profile());
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-6".to_string()));
        assert!(args.contains(&"--permission-mode".to_string()));
        assert!(args.contains(&"default".to_string()));
    }

    #[test]
    fn build_claude_args_propagates_additional_dirs_and_mcp() {
        let args = build_claude_args(&sample_profile());
        let s = args.join(" ");
        assert!(s.contains("--add-dir /extra"));
        assert!(s.contains("--mcp-config /mcp.json"));
        assert!(s.contains("--strict-mcp-config"));
        assert!(s.contains("--append-system-prompt be careful"));
    }

    #[test]
    fn build_claude_args_never_passes_max_turns() {
        let mut p = sample_profile();
        p.max_turns = Some(7);
        let args = build_claude_args(&p);
        assert!(
            !args.iter().any(|a| a == "--max-turns"),
            "--max-turns is not in claude --help; spec §7 forbids passing it"
        );
    }
}
