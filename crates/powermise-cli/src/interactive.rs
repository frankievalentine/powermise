//! Interactive harness-selection flow. Emits the same `Selection` model the
//! noninteractive `selection create` command writes, so the catalog browsing
//! below is a UI over the managed selection store rather than a parallel
//! authoring path.
use std::io::BufRead;

use powermise_domain::{AppCatalog, Catalog, MachineRole, Preferences, Selection};

/// Runs the interactive prompts and returns the assembled selection.
///
/// `input` is the prompt source (typically stdin); `machine_role` enables the
/// optional app-inclusion prompt for that role. Prompts are printed to stdout.
pub fn run_interactive_selection(
    input: &mut dyn BufRead,
    name: &str,
    catalog: &Catalog,
    apps: &AppCatalog,
    machine_role: Option<&str>,
) -> Result<Selection, String> {
    println!("harnesses available in `{}`:", catalog.catalog);
    for (index, harness) in catalog.harnesses.iter().enumerate() {
        println!("  {:2}. {} — {}", index + 1, harness.id, harness.title);
    }
    let harness_indices = prompt_ids(
        input,
        "select harnesses (comma-separated numbers, 'all', or 'none'): ",
        catalog.harnesses.len(),
    )?;
    if harness_indices.is_empty() {
        return Err("no harnesses selected; a selection requires at least one harness".into());
    }
    let harness_ids =
        map_indices_to_ids(&harness_indices, &catalog.harnesses, |harness| &harness.id);

    let mut machine_roles = std::collections::BTreeMap::new();
    if let Some(role) = machine_role {
        println!("\napps available for machine role `{role}`:");
        for (index, app) in apps.apps.iter().enumerate() {
            println!("  {:2}. {} — {}", index + 1, app.id, app.title);
        }
        let app_indices = prompt_ids(
            input,
            "include apps for this role (comma-separated numbers, 'all', or 'none'): ",
            apps.apps.len(),
        )?;
        machine_roles.insert(
            role.to_owned(),
            MachineRole {
                include_apps: map_indices_to_ids(&app_indices, &apps.apps, |app| &app.id),
                exclude_harnesses: Vec::new(),
                exclude_tools: Vec::new(),
            },
        );
    }

    // Desktop companion behavior depends on the harness mode: desktop-first
    // harnesses install their companions by default (nothing to ask), while
    // CLI-first harnesses opt in through this prompt.
    let mut preferences = Preferences::default();
    let optional_picks: Vec<&str> = harness_ids
        .iter()
        .filter_map(|id| {
            catalog
                .harnesses
                .iter()
                .find(|harness| &harness.id == id)
                .filter(|harness| !harness.desktop_tool_ids.is_empty() && !harness.desktop_required)
                .map(|harness| harness.id.as_str())
        })
        .collect();
    let required_picks: Vec<&str> = harness_ids
        .iter()
        .filter_map(|id| {
            catalog
                .harnesses
                .iter()
                .find(|harness| &harness.id == id)
                .filter(|harness| harness.desktop_required)
                .map(|harness| harness.id.as_str())
        })
        .collect();
    if !required_picks.is_empty() {
        println!(
            "\ndesktop-first harness(es) selected: {} — their desktop apps install by default",
            required_picks.join(", ")
        );
    }
    if !optional_picks.is_empty() {
        println!(
            "\ndesktop companion apps available for: {}",
            optional_picks.join(", ")
        );
        print!("also install desktop apps where available? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).map_err(|error| error.to_string())?;
        let mut answer = String::new();
        input
            .read_line(&mut answer)
            .map_err(|error| format!("could not read an answer: {error}"))?;
        preferences.include_desktop_apps = matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "ye" | "yes"
        );
    }

    Ok(Selection {
        schema_version: 1,
        name: name.to_owned(),
        catalog: catalog.catalog.clone(),
        harnesses: harness_ids,
        machine_roles,
        preferences,
    })
}

/// Maps 1-based menu indices (as returned by `prompt_ids`) to the entries'
/// identifiers. Indices are pre-validated for range, so indexing is safe.
fn map_indices_to_ids<T>(
    indices: &[String],
    entries: &[T],
    id_of: impl Fn(&T) -> &str,
) -> Vec<String> {
    indices
        .iter()
        .map(|index| {
            let position: usize = index.parse().expect("prompt_ids validated the range");
            id_of(&entries[position - 1]).to_owned()
        })
        .collect()
}

/// Parses a numbered multi-select line: comma-separated 1-based numbers,
/// `all` (every entry), or `none`/blank (nothing).
fn prompt_ids(input: &mut dyn BufRead, prompt: &str, count: usize) -> Result<Vec<String>, String> {
    print!("{prompt}");
    std::io::Write::flush(&mut std::io::stdout()).map_err(|error| error.to_string())?;

    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|error| format!("could not read a selection: {error}"))?;
    let answer = line.trim();

    if answer.is_empty() || answer.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    if answer.eq_ignore_ascii_case("all") {
        return Ok((1..=count).map(|index| index.to_string()).collect());
    }

    let mut picked = Vec::new();
    for part in answer.split(',') {
        let part = part.trim();
        let index: usize = part.parse().map_err(|_| {
            format!("`{part}` is not a number; use comma-separated numbers, 'all', or 'none'")
        })?;
        if index == 0 || index > count {
            return Err(format!("`{index}` is out of range (1..={count})"));
        }
        picked.push(index.to_string());
    }
    Ok(picked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    fn catalog() -> Catalog {
        Catalog {
            schema_version: 1,
            catalog: "powermise-profiles".into(),
            catalog_revision: "1".into(),
            harnesses: vec![
                {
                    let mut claude = harness("claude", "Claude");
                    claude.desktop_tool_ids = vec!["claude-app".into()];
                    claude
                },
                {
                    // A desktop-first harness: companion installs by default,
                    // no y/N prompt consumed.
                    let mut codex = harness("codex", "Codex");
                    codex.desktop_tool_ids = vec!["codex-app".into()];
                    codex.desktop_required = true;
                    codex
                },
                harness("gemini", "Gemini"),
            ],
        }
    }

    fn harness(id: &str, title: &str) -> powermise_domain::Harness {
        powermise_domain::Harness {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            kind: "agent".into(),
            category: "ai".into(),
            extends: None,
            supported_platforms: vec![powermise_domain::Platform::Darwin],
            required_tool_ids: Vec::new(),
            desktop_tool_ids: Vec::new(),
            desktop_required: false,
            integration_ids: Vec::new(),
            mcp_server_ids: Vec::new(),
            source_file: String::new(),
        }
    }

    fn apps() -> AppCatalog {
        AppCatalog {
            schema_version: 1,
            catalog: "powermise-profiles".into(),
            catalog_revision: "1".into(),
            apps: vec![
                powermise_domain::App {
                    id: "arc".into(),
                    title: "Arc".into(),
                    description: String::new(),
                    providers: Vec::new(),
                    unsupported_reason: None,
                },
                powermise_domain::App {
                    id: "raycast".into(),
                    title: "Raycast".into(),
                    description: String::new(),
                    providers: Vec::new(),
                    unsupported_reason: None,
                },
            ],
        }
    }

    fn run(answers: &str, role: Option<&str>) -> Result<Selection, String> {
        let mut input = BufReader::new(answers.as_bytes());
        run_interactive_selection(&mut input, "my-agent", &catalog(), &apps(), role)
    }

    #[test]
    fn picks_harnesses_by_number() {
        let selection = run("1,3\n", None).expect("selection should assemble");
        assert_eq!(
            selection.harnesses,
            vec!["claude".to_owned(), "gemini".to_owned()],
            "menu numbers must map to harness ids"
        );
        assert_eq!(selection.name, "my-agent");
        assert!(selection.machine_roles.is_empty());
    }

    #[test]
    fn all_selects_every_harness() {
        let selection = run("all\n", None).expect("selection should assemble");
        assert_eq!(
            selection.harnesses,
            vec!["claude".to_owned(), "codex".to_owned(), "gemini".to_owned()]
        );
    }

    #[test]
    fn none_harnesses_is_an_error() {
        let error = run("none\n", None).expect_err("an empty selection must be rejected");
        assert!(error.contains("at least one harness"));
    }

    #[test]
    fn machine_role_collects_apps() {
        let selection = run("1\n1,2\n", Some("macbook")).expect("selection should assemble");
        let role = selection
            .machine_roles
            .get("macbook")
            .expect("role should be recorded");
        assert_eq!(role.include_apps, vec!["arc", "raycast"]);
    }

    #[test]
    fn desktop_companions_are_opt_in_via_prompt() {
        // The fixture `claude` harness ships a desktop companion; the "y"
        // answer must opt the selection in.
        let selection = run("1\ny\n", None).expect("selection should assemble");
        assert!(selection.preferences.include_desktop_apps);
        // A non-affirmative answer keeps desktop apps off.
        let selection = run("1\nn\n", None).expect("selection should assemble");
        assert!(!selection.preferences.include_desktop_apps);
    }

    #[test]
    fn desktop_prompt_is_skipped_without_companions() {
        // The fixture `gemini` harness has no desktop companions, so a single
        // answer line must suffice.
        let selection = run("3\n", None).expect("selection should assemble");
        assert_eq!(selection.harnesses, vec!["gemini".to_owned()]);
        assert!(!selection.preferences.include_desktop_apps);
    }

    #[test]
    fn desktop_first_harness_needs_no_opt_in_answer() {
        // The fixture `codex` harness is desktop-first: selecting it consumes
        // no prompt line and leaves the opt-in preference untouched — the
        // companion resolves by default through the registry.
        let selection = run("2\n", None).expect("selection should assemble");
        assert_eq!(selection.harnesses, vec!["codex".to_owned()]);
        assert!(!selection.preferences.include_desktop_apps);
    }

    #[test]
    fn out_of_range_index_is_rejected() {
        let error = run("7\n", None).expect_err("an out-of-range index must be rejected");
        assert!(error.contains("out of range"));
    }

    #[test]
    fn garbage_input_is_rejected() {
        let error = run("claude\n", None).expect_err("non-numeric input must be rejected");
        assert!(error.contains("is not a number"));
    }
}
