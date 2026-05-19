//! L1 round-trip integration tests against the public Rust API.
//!
//! These test the same flows the IPC layer exercises, but call the Rust
//! services directly so we don't need a Tauri runtime.

use std::path::PathBuf;

use chrono::Utc;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use promptibrary_lib::domain::prompt::{
    ClaudeModelId, ClaudePermissionMode, LaunchDefaults, LaunchDestination, Prompt,
    PromptTelemetrySummary, VerifierMode,
};
use promptibrary_lib::domain::source::{ManualSource, Source};
use promptibrary_lib::domain::variable::Variable;
use promptibrary_lib::ids::PromptId;
use promptibrary_lib::index::migrations::run_migrations;
use promptibrary_lib::index::prompts_repo;
use promptibrary_lib::vault::paths::{prompt_path_for_slug, VaultPaths};
use promptibrary_lib::vault::repair::repair_missing_dirs;
use promptibrary_lib::vault::scanner::scan_vault;
use promptibrary_lib::vault::writer::write_prompt;
use promptibrary_lib::variables::parser::{parse_template_variables, ParseVariablesInput};
use promptibrary_lib::variables::renderer::{
    render_prompt, BoolRenderMode, RenderPromptInput, ResolvedVariableValue,
};

async fn temp_db() -> SqlitePool {
    // SCA-598: in-memory SQLite — was tempfile + mem::forget which leaked
    // a directory per integration-test run.
    let opts = promptibrary_lib::index::db::in_memory_connect_options();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    pool
}

fn default_launch_defaults() -> LaunchDefaults {
    LaunchDefaults {
        destination: LaunchDestination::ClaudeCodeCli,
        model: ClaudeModelId::ClaudeSonnet46,
        verifier_mode: VerifierMode::Off,
        working_directory: None,
        additional_directories: vec![],
        permission_mode: ClaudePermissionMode::Default,
        allowed_tools: vec![],
        disallowed_tools: vec![],
        mcp_config_paths: vec![],
        strict_mcp_config: false,
        append_system_prompt: None,
        max_turns: None,
    }
}

fn make_prompt(title: &str, slug: &str, body: &str) -> Prompt {
    Prompt {
        id: PromptId(promptibrary_lib::ids::new_ulid()),
        title: title.into(),
        slug: slug.into(),
        summary: "round-trip test".into(),
        body: body.into(),
        vault_path: prompt_path_for_slug(slug),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        archived_at: None,
        tags: vec!["test".into()],
        source: Source::Manual(ManualSource {
            title: None,
            author: None,
            fetched_at: None,
            content_hash: None,
        }),
        variables: vec![],
        launch_defaults: default_launch_defaults(),
        telemetry: PromptTelemetrySummary {
            launch_count: 0,
            last_used_at: None,
            success_rate: None,
            avg_run_seconds: None,
            avg_token_count: None,
        },
        checksum_sha256: String::new(),
    }
}

#[tokio::test]
async fn round_trip_write_scan_get_render() {
    let vault_dir = tempfile::tempdir().unwrap();
    let vault = VaultPaths::new(vault_dir.path());
    repair_missing_dirs(&vault).unwrap();
    let db = temp_db().await;

    let body = "Refactor {{file:target}} inside {{folder:root}}.\nNotes:\n{{multiline:notes}}\n";
    let mut p = make_prompt("All vars", "all-vars", body);
    write_prompt(&vault, &mut p).unwrap();
    assert!(vault.absolute(&p.vault_path).unwrap().exists());

    // Scan picks it up.
    let summary = scan_vault(&vault, &db, |_| {}).await.unwrap();
    assert_eq!(summary.scanned_files, 1);
    assert_eq!(summary.indexed_prompts, 1);

    // Index row reflects the file.
    let row = prompts_repo::get_prompt_index(&db, p.id.as_str())
        .await
        .unwrap()
        .expect("indexed");
    assert_eq!(row.vault_path, p.vault_path);

    // Parse + render the body.
    let parsed = parse_template_variables(ParseVariablesInput {
        template: body.to_string(),
        frontmatter_variables: vec![],
    });
    assert_eq!(parsed.refs.len(), 3);
    let rendered = render_prompt(RenderPromptInput {
        template: body.to_string(),
        refs: parsed.refs,
        values: vec![
            ResolvedVariableValue::File {
                key: "target".into(),
                value: PathBuf::from("/tmp/x.rs"),
            },
            ResolvedVariableValue::Folder {
                key: "root".into(),
                value: PathBuf::from("/tmp/proj"),
            },
            ResolvedVariableValue::Multiline {
                key: "notes".into(),
                value: "- be careful\n- run tests".into(),
            },
        ],
        bool_render_mode: BoolRenderMode::FrontmatterStrings,
        variables: vec![],
    });
    assert!(rendered.errors.is_empty(), "errors: {:?}", rendered.errors);
    assert!(rendered.rendered.contains("/tmp/x.rs"));
    assert!(rendered.rendered.contains("/tmp/proj"));
    assert!(rendered.rendered.contains("- run tests"));
}

#[tokio::test]
async fn slug_collision_appends_dash_two() {
    let vault_dir = tempfile::tempdir().unwrap();
    let vault = VaultPaths::new(vault_dir.path());
    repair_missing_dirs(&vault).unwrap();
    let db = temp_db().await;

    let mut a = make_prompt("Refactor module", "refactor-module", "body A");
    let mut b = make_prompt("Refactor module", "refactor-module", "body B");
    // The IPC layer would assign `refactor-module-2` via prompts_repo::slug_in_use.
    // Simulate that here.
    write_prompt(&vault, &mut a).unwrap();
    prompts_repo::upsert_prompt(&db, &a).await.unwrap();
    let next_slug = if prompts_repo::slug_in_use(&db, "refactor-module").await.unwrap() {
        "refactor-module-2".to_string()
    } else {
        "refactor-module".to_string()
    };
    b.slug = next_slug.clone();
    b.vault_path = prompt_path_for_slug(&next_slug);
    write_prompt(&vault, &mut b).unwrap();
    prompts_repo::upsert_prompt(&db, &b).await.unwrap();

    assert_ne!(a.id.0, b.id.0);
    assert_eq!(b.slug, "refactor-module-2");
}

#[tokio::test]
async fn archive_excludes_from_default_list() {
    let vault_dir = tempfile::tempdir().unwrap();
    let vault = VaultPaths::new(vault_dir.path());
    repair_missing_dirs(&vault).unwrap();
    let db = temp_db().await;

    let mut alive = make_prompt("Alive", "alive", "x");
    let mut dead = make_prompt("Archived", "archived", "y");
    dead.archived_at = Some(Utc::now());
    write_prompt(&vault, &mut alive).unwrap();
    write_prompt(&vault, &mut dead).unwrap();
    prompts_repo::upsert_prompt(&db, &alive).await.unwrap();
    prompts_repo::upsert_prompt(&db, &dead).await.unwrap();

    let active = prompts_repo::list_prompts_for_library(
        &db,
        prompts_repo::LibraryFilters::default(),
    )
    .await
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, alive.id.0);

    let all = prompts_repo::list_prompts_for_library(
        &db,
        prompts_repo::LibraryFilters {
            include_archived: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(all.len(), 2);
}
