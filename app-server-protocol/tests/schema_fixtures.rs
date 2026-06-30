use agere_app_server_protocol::GenerateTsOptions;
use agere_app_server_protocol::generate_json_with_experimental;
use agere_app_server_protocol::generate_ts_with_options;
use agere_app_server_protocol::generate_typescript_schema_fixture_subtree_for_tests;
use agere_app_server_protocol::read_schema_fixture_subtree;
use anyhow::Context;
use anyhow::Result;
use similar::TextDiff;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

#[test]
fn typescript_schema_fixtures_match_generated() -> Result<()> {
    let schema_root = schema_root()?;
    let fixture_tree = read_tree(&schema_root, "typescript")?;
    let generated_tree = generate_typescript_schema_fixture_subtree_for_tests()
        .context("generate in-memory typescript schema fixtures")?;

    assert_schema_trees_match("typescript", &fixture_tree, &generated_tree)?;

    Ok(())
}

#[test]
fn json_schema_fixtures_match_generated() -> Result<()> {
    assert_schema_fixtures_match_generated("json", |output_dir| {
        generate_json_with_experimental(output_dir, /*experimental_api*/ false)
    })
}

#[test]
fn experimental_schema_generation_includes_initial_turns_page() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temp dir")?;
    let typescript_dir = temp_dir.path().join("typescript");
    generate_ts_with_options(
        &typescript_dir,
        None,
        GenerateTsOptions {
            experimental_api: true,
            ..Default::default()
        },
    )
    .context("generate experimental TypeScript schema fixtures")?;

    let thread_resume_params_ts =
        std::fs::read_to_string(typescript_dir.join("v2").join("ThreadResumeParams.ts"))?;
    assert!(thread_resume_params_ts.contains("initialTurnsPage"));
    let thread_resume_response_ts =
        std::fs::read_to_string(typescript_dir.join("v2").join("ThreadResumeResponse.ts"))?;
    assert!(thread_resume_response_ts.contains("initialTurnsPage"));

    let json_dir = temp_dir.path().join("json");
    generate_json_with_experimental(&json_dir, /*experimental_api*/ true)
        .context("generate experimental JSON schema fixtures")?;
    let thread_resume_params_json =
        std::fs::read_to_string(json_dir.join("v2").join("ThreadResumeParams.json"))?;
    assert!(thread_resume_params_json.contains("initialTurnsPage"));
    let thread_resume_response_json =
        std::fs::read_to_string(json_dir.join("v2").join("ThreadResumeResponse.json"))?;
    assert!(thread_resume_response_json.contains("initialTurnsPage"));

    Ok(())
}

#[test]
fn stable_schema_generation_excludes_initial_turns_page_helpers() -> Result<()> {
    let temp_dir = tempfile::tempdir().context("create temp dir")?;
    let typescript_dir = temp_dir.path().join("typescript");
    generate_ts_with_options(
        &typescript_dir,
        None,
        GenerateTsOptions {
            run_prettier: false,
            ..Default::default()
        },
    )
    .context("generate stable TypeScript schema fixtures")?;

    let index_ts = std::fs::read_to_string(typescript_dir.join("v2").join("index.ts"))?;
    assert!(!index_ts.contains("ThreadResumeInitialTurnsPageParams"));
    assert!(!index_ts.contains("TurnsPage"));
    assert!(
        !typescript_dir
            .join("v2")
            .join("ThreadResumeInitialTurnsPageParams.ts")
            .exists()
    );
    assert!(!typescript_dir.join("v2").join("TurnsPage.ts").exists());

    let json_dir = temp_dir.path().join("json");
    generate_json_with_experimental(&json_dir, /*experimental_api*/ false)
        .context("generate stable JSON schema fixtures")?;
    let bundle_json =
        std::fs::read_to_string(json_dir.join("agere_app_server_protocol.schemas.json"))?;
    assert!(!bundle_json.contains("ThreadResumeInitialTurnsPageParams"));
    assert!(!bundle_json.contains("TurnsPage"));

    Ok(())
}

fn assert_schema_fixtures_match_generated(
    label: &'static str,
    generate: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let schema_root = schema_root()?;
    let fixture_tree = read_tree(&schema_root, label)?;

    let temp_dir = tempfile::tempdir().context("create temp dir")?;
    let generated_root = temp_dir.path().join(label);
    generate(&generated_root).with_context(|| {
        format!(
            "generate {label} schema fixtures into {}",
            generated_root.display()
        )
    })?;

    let generated_tree = read_tree(temp_dir.path(), label)?;

    assert_schema_trees_match(label, &fixture_tree, &generated_tree)?;

    Ok(())
}

fn assert_schema_trees_match(
    label: &str,
    fixture_tree: &BTreeMap<PathBuf, Vec<u8>>,
    generated_tree: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<()> {
    let fixture_paths = fixture_tree
        .keys()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();
    let generated_paths = generated_tree
        .keys()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>();

    if fixture_paths != generated_paths {
        let expected = fixture_paths.join("\n");
        let actual = generated_paths.join("\n");
        let diff = TextDiff::from_lines(&expected, &actual)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();

        panic!(
            "Vendored {label} app-server schema fixture file set doesn't match freshly generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}"
        );
    }

    // If the file sets match, diff contents for each file for a nicer error.
    for (path, expected) in fixture_tree {
        let actual = generated_tree
            .get(path)
            .ok_or_else(|| anyhow::anyhow!("missing generated file: {}", path.display()))?;

        if expected == actual {
            continue;
        }

        let expected_str = String::from_utf8_lossy(expected);
        let actual_str = String::from_utf8_lossy(actual);
        let diff = TextDiff::from_lines(&expected_str, &actual_str)
            .unified_diff()
            .header("fixture", "generated")
            .to_string();
        panic!(
            "Vendored {label} app-server schema fixture {} differs from generated output. \
Run `just write-app-server-schema` to overwrite with your changes.\n\n{diff}",
            path.display()
        );
    }

    Ok(())
}

fn schema_root() -> Result<PathBuf> {
    // In Bazel runfiles (especially manifest-only mode), resolving directories is not
    // reliable. Resolve a known file, then walk up to the schema root.
    let typescript_index = agere_utils_cargo_bin::find_resource!("schema/typescript/index.ts")
        .context("resolve TypeScript schema index.ts")?;
    let schema_root = typescript_index
        .parent()
        .and_then(|p| p.parent())
        .context("derive schema root from schema/typescript/index.ts")?
        .to_path_buf();

    // Sanity check that the JSON fixtures resolve to the same schema root.
    let json_bundle =
        agere_utils_cargo_bin::find_resource!("schema/json/agere_app_server_protocol.schemas.json")
            .context("resolve JSON schema bundle")?;
    let json_root = json_bundle
        .parent()
        .and_then(|p| p.parent())
        .context("derive schema root from schema/json/agere_app_server_protocol.schemas.json")?;
    anyhow::ensure!(
        schema_root == json_root,
        "schema roots disagree: typescript={} json={}",
        schema_root.display(),
        json_root.display()
    );

    Ok(schema_root)
}

fn read_tree(root: &Path, label: &str) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    read_schema_fixture_subtree(root, label).with_context(|| {
        format!(
            "read {label} schema fixture subtree from {}",
            root.display()
        )
    })
}
