//! `skills` verb — serve embedded agent runbooks, plus optional on-disk
//! skill-data dirs declared per-repo in `agent-qa.toml`.
//!
//! Layout assumption (mirrors `skill-data/` at repo root, embedded at build
//! time via `include_dir!`):
//!
//! ```
//! skill-data/
//! ├── <name>/SKILL.md          ← primary content
//! └── <name>/references/*.md   ← supporting references (skills get returns SKILL.md only)
//! ```
//!
//! External (per-repo) skills are declared in `agent-qa.toml`, walked up
//! from cwd (same discovery as `[plugins]`):
//!
//! ```toml
//! [skills]
//! extra-dirs = [
//!   "./packages/ingestion/scenario-runner/skill-data",
//! ]
//! ```
//!
//! Each listed dir is scanned for `<name>/SKILL.md`. Embedded skills always
//! win on name collision (the binary is the source of truth for `core` /
//! `byo` / `profiles`).
//!
//! Verbs:
//!   skills                       → list (default)
//!   skills list                  → list skill names, one per line
//!   skills get <name>            → print SKILL.md for the skill
//!   skills path [name]           → print path; with no arg, prints all paths
//!   skills scaffold <name>       → write a template SKILL.md for a new
//!                                  downstream skill and register the
//!                                  containing dir in agent-qa.toml
//!
//! Embedded README.md at the root is intentionally skipped from the listing.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use include_dir::{include_dir, Dir};
use serde::Deserialize;

static SKILL_DATA: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../skill-data");

/// Where a skill came from.
#[derive(Debug, Clone)]
enum Source {
    /// Embedded in the binary at compile time. `path` is the logical
    /// repo-relative path (`skill-data/<name>/SKILL.md`).
    Embedded { path: String },
    /// On-disk SKILL.md discovered via `agent-qa.toml [skills] extra-dirs`.
    External { skill_md: PathBuf },
}

#[derive(Debug, Clone)]
struct Skill {
    name: String,
    source: Source,
}

pub fn run(args: &[String]) -> Result<u8> {
    let json_out = args.iter().any(|a| a == "--json");
    let rest: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|s| *s != "--json")
        .collect();
    match rest.first().copied() {
        None | Some("list") => list(json_out),
        Some("get") => {
            let name = rest
                .get(1)
                .copied()
                .ok_or_else(|| anyhow!("usage: skills get <name>"))?;
            get(name)
        }
        Some("path") => path(rest.get(1).copied()),
        Some("scaffold") => {
            let name = rest
                .get(1)
                .copied()
                .ok_or_else(|| anyhow!("usage: skills scaffold <name> [--dir <path>]"))?;
            let dir = parse_flag(&rest, "--dir");
            scaffold(name, dir.as_deref())
        }
        Some("--help" | "-h" | "help") => {
            help();
            Ok(0)
        }
        Some(other) => bail!("unknown subverb {other:?}. Try: list | get <name> | path [name]"),
    }
}

fn help() {
    println!(
        "agent-qa skills — serve embedded + per-repo agent runbooks

Usage:
  agent-qa skills [list]      List skill names
  agent-qa skills list --json Structured rows: [{{name, source, path}}]
  agent-qa skills get <name>  Print the skill's SKILL.md
  agent-qa skills path [name] Print the path for a skill (or all)
  agent-qa skills scaffold <name> [--dir <path>]
                              Write a template SKILL.md for a new
                              downstream skill (pages, glossary, auth, …)
                              and register its parent dir in agent-qa.toml.

Discovery:
  1. Embedded skills compiled into the binary (core, byo, profiles).
  2. On-disk dirs declared in agent-qa.toml (walked up from cwd):

       [skills]
       extra-dirs = [\"./path/to/skill-data\"]

  Each dir is scanned for <name>/SKILL.md. Embedded skills win on name
  collision."
    );
}

fn list(json_out: bool) -> Result<u8> {
    let skills = discover()?;
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            name: String,
            source: &'static str,
            path: String,
        }
        let rows: Vec<Row> = skills
            .iter()
            .map(|s| match &s.source {
                Source::Embedded { path } => Row {
                    name: s.name.clone(),
                    source: "embedded",
                    path: path.clone(),
                },
                Source::External { skill_md } => Row {
                    name: s.name.clone(),
                    source: "external",
                    path: skill_md.display().to_string(),
                },
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(0);
    }
    for s in skills {
        println!("{}", s.name);
    }
    Ok(0)
}

fn get(name: &str) -> Result<u8> {
    let skills = discover()?;
    let skill = skills
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| anyhow!("unknown skill {name:?}. Try `agent-qa skills list`."))?;
    use std::io::Write;
    let bytes = match &skill.source {
        Source::Embedded { path } => SKILL_DATA
            .get_file(path.trim_start_matches("skill-data/"))
            .expect("embedded skill present in discover()")
            .contents()
            .to_vec(),
        Source::External { skill_md } => fs::read(skill_md)
            .with_context(|| format!("read external skill {}", skill_md.display()))?,
    };
    std::io::stdout().write_all(&bytes)?;
    Ok(0)
}

fn path(name: Option<&str>) -> Result<u8> {
    let skills = discover()?;
    match name {
        Some(name) => {
            let skill = skills
                .iter()
                .find(|s| s.name == name)
                .ok_or_else(|| anyhow!("unknown skill {name:?}"))?;
            println!("{}", render_path(&skill.source));
            Ok(0)
        }
        None => {
            for s in skills {
                println!("{}", render_path(&s.source));
            }
            Ok(0)
        }
    }
}

fn parse_flag(args: &[&str], flag: &str) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if *a == flag {
            return it.next().map(|s| s.to_string());
        }
        if let Some(rest) = a.strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

/// Create a starter SKILL.md for a new downstream skill and ensure the
/// containing dir is registered in `agent-qa.toml`. Refuses to overwrite
/// existing SKILL.md files. Generic — `pages` is just the canonical
/// example; this works for any skill name.
fn scaffold(name: &str, dir_override: Option<&str>) -> Result<u8> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.starts_with('.')
        || name.starts_with('-')
    {
        bail!("invalid skill name {name:?}: must be a single path segment, no slashes, no leading dot/dash");
    }
    let cwd = env::current_dir().context("get cwd")?;
    scaffold_in(&cwd, name, dir_override)?;
    Ok(0)
}

fn scaffold_in(cwd: &Path, name: &str, dir_override: Option<&str>) -> Result<PathBuf> {
    // Resolve the parent skill-data dir.
    //
    // Precedence:
    //   1. --dir <path> override (relative to cwd, or absolute).
    //   2. First extra-dir from the nearest agent-qa.toml walking up from cwd.
    //   3. Default: ./skill-data (registered into ./agent-qa.toml).
    let existing_toml = load_config_walking_up(cwd)?;
    let (parent_dir, toml_to_update, dir_rel_for_toml) = match dir_override {
        Some(spec) => {
            let p = PathBuf::from(spec);
            let abs = if p.is_absolute() { p } else { cwd.join(p) };
            // Register against the nearest agent-qa.toml or, if none,
            // create one in cwd.
            let (toml_path, rel) = match &existing_toml {
                Some((toml_path, _)) => {
                    let base = toml_path.parent().unwrap_or(cwd);
                    let rel = pathdiff::diff_paths(&abs, base)
                        .map(|p| format!("./{}", p.display()))
                        .unwrap_or_else(|| abs.display().to_string());
                    (toml_path.clone(), rel)
                }
                None => {
                    let toml_path = cwd.join("agent-qa.toml");
                    let rel = pathdiff::diff_paths(&abs, cwd)
                        .map(|p| format!("./{}", p.display()))
                        .unwrap_or_else(|| abs.display().to_string());
                    (toml_path, rel)
                }
            };
            (abs, toml_path, rel)
        }
        None => match &existing_toml {
            Some((toml_path, cfg)) => {
                let base = toml_path.parent().unwrap_or(cwd).to_path_buf();
                let first = cfg
                    .skills
                    .as_ref()
                    .and_then(|s| s.extra_dirs.as_ref())
                    .and_then(|v| v.first().cloned());
                match first {
                    Some(spec) => {
                        let expanded = crate::global_config::expand_tilde(&spec);
                        let abs = if expanded.is_absolute() {
                            expanded
                        } else {
                            base.join(expanded)
                        };
                        (abs, toml_path.clone(), spec)
                    }
                    None => {
                        let abs = base.join("skill-data");
                        (abs, toml_path.clone(), "./skill-data".to_string())
                    }
                }
            }
            None => {
                let abs = cwd.join("skill-data");
                (abs, cwd.join("agent-qa.toml"), "./skill-data".to_string())
            }
        },
    };

    let skill_dir = parent_dir.join(name);
    let skill_md = skill_dir.join("SKILL.md");
    if skill_md.exists() {
        bail!(
            "refusing to overwrite existing skill at {}",
            skill_md.display()
        );
    }
    fs::create_dir_all(&skill_dir).with_context(|| format!("create {}", skill_dir.display()))?;
    fs::write(&skill_md, scaffold_template(name))
        .with_context(|| format!("write {}", skill_md.display()))?;

    let registered = ensure_extra_dir_registered(&toml_to_update, &dir_rel_for_toml)?;

    eprintln!("created {}", skill_md.display());
    if registered {
        eprintln!(
            "registered {dir_rel_for_toml:?} in [skills].extra-dirs of {}",
            toml_to_update.display()
        );
    } else {
        eprintln!(
            "{dir_rel_for_toml:?} already registered in {}",
            toml_to_update.display()
        );
    }
    eprintln!("next: edit the SKILL.md, fill in the description + body, then `agent-qa skills get {name}`");
    Ok(skill_md)
}

fn scaffold_template(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: TODO — describe when an agent should read this skill. Be\n  specific about *when* to load it; vague descriptions get ignored.\n---\n\n# {name}\n\nTODO — replace this body with your product-specific knowledge.\n\nCommon shapes for a downstream skill:\n\n- **Route map** (e.g. `pages`): named pages → URL patterns the agent should\n  use instead of guessing.\n- **Glossary**: in-house terminology + canonical spellings.\n- **Auth / setup runbook**: how to bootstrap a session before recording.\n- **Feature-flag atlas**: flag names + what they gate.\n\nKeep this file focused. If it grows beyond one screen, split supporting\nmaterial into a `references/` subdirectory next to this SKILL.md.\n"
    )
}

/// Append `dir_rel` to `[skills].extra-dirs` in `toml_path` if not already
/// present. Returns `true` if a change was written. Creates the file with a
/// minimal `[skills]` section if it does not exist.
fn ensure_extra_dir_registered(toml_path: &Path, dir_rel: &str) -> Result<bool> {
    let existing = fs::read_to_string(toml_path).ok();
    let already = existing
        .as_deref()
        .and_then(|s| toml::from_str::<ConfigFile>(s).ok())
        .and_then(|c| c.skills)
        .and_then(|s| s.extra_dirs)
        .map(|dirs| dirs.iter().any(|d| d == dir_rel))
        .unwrap_or(false);
    if already {
        return Ok(false);
    }
    let new_contents = match existing {
        None => {
            format!("[skills]\nextra-dirs = [\"{dir_rel}\"]\n")
        }
        Some(prev) => append_extra_dir(&prev, dir_rel),
    };
    if let Some(parent) = toml_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::write(toml_path, new_contents).with_context(|| format!("write {}", toml_path.display()))?;
    Ok(true)
}

/// Best-effort textual append into `[skills].extra-dirs`. Handles three
/// cases without a full TOML round-trip (which would lose comments):
///   1. No `[skills]` section at all → append one.
///   2. `[skills]` present without `extra-dirs` → insert the key.
///   3. `extra-dirs = [...]` present → append the entry inside the array.
fn append_extra_dir(prev: &str, dir_rel: &str) -> String {
    // Case 3: there is an existing extra-dirs array.
    if let Some(idx) = find_extra_dirs_array(prev) {
        let (head, tail) = prev.split_at(idx);
        if let Some(close) = tail.find(']') {
            let array_body = &tail[..close];
            let after = &tail[close..];
            let trimmed = array_body.trim_end();
            let needs_comma = !trimmed.ends_with('[') && !trimmed.ends_with(',');
            let sep = if needs_comma { ", " } else { " " };
            return format!("{head}{trimmed}{sep}\"{dir_rel}\"{after}");
        }
    }
    // Case 2: [skills] section without the key.
    if let Some(sect_idx) = prev.find("[skills]") {
        // Insert directly after the section header line.
        let after = sect_idx + "[skills]".len();
        let (head, tail) = prev.split_at(after);
        // Skip to end of header line.
        let nl = tail.find('\n').map(|n| n + 1).unwrap_or(tail.len());
        let (line_end, rest) = tail.split_at(nl);
        return format!("{head}{line_end}extra-dirs = [\"{dir_rel}\"]\n{rest}");
    }
    // Case 1: no [skills] section.
    let sep = if prev.ends_with('\n') || prev.is_empty() {
        ""
    } else {
        "\n"
    };
    format!("{prev}{sep}\n[skills]\nextra-dirs = [\"{dir_rel}\"]\n")
}

fn find_extra_dirs_array(s: &str) -> Option<usize> {
    for key in ["extra-dirs", "extra_dirs"] {
        if let Some(k) = s.find(key) {
            let after = &s[k + key.len()..];
            if let Some(eq_off) = after.find('=') {
                let after_eq = &after[eq_off + 1..];
                if let Some(bracket_off) = after_eq.find('[') {
                    let between = &after_eq[..bracket_off];
                    if between.trim().is_empty() {
                        let abs = k + key.len() + eq_off + 1 + bracket_off + 1;
                        return Some(abs);
                    }
                }
            }
        }
    }
    None
}

fn render_path(src: &Source) -> String {
    match src {
        Source::Embedded { path } => path.clone(),
        Source::External { skill_md } => skill_md.display().to_string(),
    }
}

/// Discover the full skill catalogue: embedded first (binary is the source
/// of truth), then external dirs declared in `agent-qa.toml`. Sorted by name.
fn discover() -> Result<Vec<Skill>> {
    let cwd = env::current_dir().context("get cwd")?;
    Ok(discover_from(&cwd))
}

fn discover_from(cwd: &Path) -> Vec<Skill> {
    let mut by_name: BTreeMap<String, Skill> = BTreeMap::new();

    // 1. Embedded — wins on collision.
    for name in embedded_skill_names() {
        by_name.insert(
            name.clone(),
            Skill {
                name: name.clone(),
                source: Source::Embedded {
                    path: format!("skill-data/{name}/SKILL.md"),
                },
            },
        );
    }

    // 2. External dirs unioned across:
    //      a) global config(s)  (~/.agent-qa/agent-qa.toml, XDG fallback)
    //      b) per-repo agent-qa.toml walked up from cwd
    //    Both contribute extra-dirs; embedded names still win on collision.
    let mut configs: Vec<PathBuf> = crate::global_config::existing_global_config_files();
    if let Ok(Some((toml_path, _))) = load_config_walking_up(cwd) {
        configs.push(toml_path);
    }

    for toml_path in configs {
        let bytes = match fs::read_to_string(&toml_path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let cfg: ConfigFile = match toml::from_str(&bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let base = toml_path.parent().unwrap_or_else(|| Path::new("."));
        let extra_dirs = cfg.skills.and_then(|s| s.extra_dirs).unwrap_or_default();
        for spec in extra_dirs {
            let expanded = crate::global_config::expand_tilde(&spec);
            let dir = if expanded.is_absolute() {
                expanded
            } else {
                base.join(expanded)
            };
            let entries = match fs::read_dir(&dir) {
                Ok(it) => it,
                Err(_) => continue, // silently skip missing dirs
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                let skill_md = p.join("SKILL.md");
                if !skill_md.is_file() {
                    continue;
                }
                let name = match p.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                by_name.entry(name.clone()).or_insert(Skill {
                    name,
                    source: Source::External { skill_md },
                });
            }
        }
    }

    by_name.into_values().collect()
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    skills: Option<SkillsSection>,
}

#[derive(Debug, Deserialize)]
struct SkillsSection {
    #[serde(rename = "extra-dirs", alias = "extra_dirs")]
    extra_dirs: Option<Vec<String>>,
}

fn load_config_walking_up(start: &Path) -> Result<Option<(PathBuf, ConfigFile)>> {
    let mut cur: Option<&Path> = Some(start);
    while let Some(dir) = cur {
        for name in ["agent-qa.toml", ".agent-qa.toml"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                let bytes = fs::read_to_string(&candidate)
                    .with_context(|| format!("read {}", candidate.display()))?;
                // Tolerate other sections (e.g. [plugins]) — only [skills] is
                // our concern here. Unknown fields are ignored by serde.
                let parsed: ConfigFile = toml::from_str(&bytes)
                    .with_context(|| format!("parse {}", candidate.display()))?;
                return Ok(Some((candidate, parsed)));
            }
        }
        cur = dir.parent();
    }
    Ok(None)
}

/// Embedded skill names — every immediate subdirectory of `skill-data/` that
/// contains a `SKILL.md`. The root `README.md` is skipped.
fn embedded_skill_names() -> Vec<String> {
    let mut names: Vec<String> = SKILL_DATA
        .dirs()
        .filter_map(|d| {
            let name = d.path().file_name()?.to_string_lossy().into_owned();
            let skill_md = format!("{name}/SKILL.md");
            SKILL_DATA.get_file(&skill_md).map(|_| name)
        })
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn list_includes_known_embedded_skills() {
        let names: Vec<String> = discover_from(&PathBuf::from("/"))
            .into_iter()
            .map(|s| s.name)
            .collect();
        for want in ["core", "byo", "profiles"] {
            assert!(
                names.contains(&want.to_string()),
                "missing {want} in {names:?}"
            );
        }
    }

    #[test]
    fn get_unknown_skill_errors() {
        let err = get("does-not-exist").unwrap_err();
        assert!(err.to_string().contains("unknown skill"));
    }

    #[test]
    fn external_skills_discovered_via_agent_qa_toml() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // External dir with two skills.
        let extra = root.join("packages/ingestion/scenario-runner/skill-data");
        fs::create_dir_all(extra.join("pages")).unwrap();
        fs::create_dir_all(extra.join("acme")).unwrap();
        fs::write(extra.join("pages/SKILL.md"), "# pages\nbody").unwrap();
        fs::write(extra.join("acme/SKILL.md"), "# acme\nbody").unwrap();

        fs::write(
            root.join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./packages/ingestion/scenario-runner/skill-data\"]\n",
        )
        .unwrap();

        let names: Vec<String> = discover_from(root).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"pages".to_string()), "{names:?}");
        assert!(names.contains(&"acme".to_string()), "{names:?}");
        // Embedded still present.
        assert!(names.contains(&"core".to_string()), "{names:?}");
    }

    #[test]
    fn embedded_wins_on_name_collision() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // External tries to shadow `core`.
        let extra = root.join("extras");
        fs::create_dir_all(extra.join("core")).unwrap();
        fs::write(extra.join("core/SKILL.md"), "SHOULD NOT WIN").unwrap();
        fs::write(
            root.join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./extras\"]\n",
        )
        .unwrap();

        let skills = discover_from(root);
        let core = skills.iter().find(|s| s.name == "core").unwrap();
        assert!(matches!(core.source, Source::Embedded { .. }));
    }

    #[test]
    fn missing_extra_dir_is_silently_skipped() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./does/not/exist\"]\n",
        )
        .unwrap();
        // Must not panic.
        let names: Vec<String> = discover_from(tmp.path())
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"core".to_string()));
    }

    #[test]
    fn toml_with_only_plugins_section_is_ignored_for_skills() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[plugins]\nauth = \"/some/binary\"\n",
        )
        .unwrap();
        // Should not crash on missing [skills].
        let names: Vec<String> = discover_from(tmp.path())
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"core".to_string()));
    }

    #[test]
    fn global_config_extra_dirs_apply_outside_any_repo() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cfg_dir = home.join(".agent-qa");
        fs::create_dir_all(&cfg_dir).unwrap();
        let extra = tmp.path().join("globals");
        fs::create_dir_all(extra.join("globalskill")).unwrap();
        fs::write(extra.join("globalskill/SKILL.md"), "# x").unwrap();
        fs::write(
            cfg_dir.join("agent-qa.toml"),
            format!("[skills]\nextra-dirs = ['{}']\n", extra.display()),
        )
        .unwrap();

        std::env::set_var("HOME", &home);
        std::env::remove_var("USERPROFILE");
        std::env::remove_var("XDG_CONFIG_HOME");
        // cwd is *not* under home and contains no agent-qa.toml — only the
        // global config should contribute.
        let cwd = tmp.path().join("elsewhere");
        fs::create_dir_all(&cwd).unwrap();
        let names: Vec<String> = discover_from(&cwd).into_iter().map(|s| s.name).collect();
        std::env::remove_var("HOME");
        assert!(names.contains(&"globalskill".to_string()), "{names:?}");
    }

    #[test]
    fn tilde_in_extra_dirs_resolves_against_home() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let extra = home.join(".agent-qa").join("skills");
        fs::create_dir_all(extra.join("tildeskill")).unwrap();
        fs::write(extra.join("tildeskill/SKILL.md"), "# t").unwrap();
        let cfg_dir = home.join(".agent-qa");
        fs::write(
            cfg_dir.join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"~/.agent-qa/skills\"]\n",
        )
        .unwrap();

        std::env::set_var("HOME", &home);
        std::env::remove_var("USERPROFILE");
        std::env::remove_var("XDG_CONFIG_HOME");
        let cwd = tmp.path().join("elsewhere");
        fs::create_dir_all(&cwd).unwrap();
        let names: Vec<String> = discover_from(&cwd).into_iter().map(|s| s.name).collect();
        std::env::remove_var("HOME");
        assert!(names.contains(&"tildeskill".to_string()), "{names:?}");
    }

    #[test]
    fn global_and_repo_extra_dirs_are_unioned() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cfg_dir = home.join(".agent-qa");
        fs::create_dir_all(&cfg_dir).unwrap();
        let g_extra = tmp.path().join("g-extras");
        fs::create_dir_all(g_extra.join("g_skill")).unwrap();
        fs::write(g_extra.join("g_skill/SKILL.md"), "# g").unwrap();
        fs::write(
            cfg_dir.join("agent-qa.toml"),
            format!("[skills]\nextra-dirs = ['{}']\n", g_extra.display()),
        )
        .unwrap();

        let repo = tmp.path().join("repo");
        let r_extra = repo.join("r-extras");
        fs::create_dir_all(r_extra.join("r_skill")).unwrap();
        fs::write(r_extra.join("r_skill/SKILL.md"), "# r").unwrap();
        fs::write(
            repo.join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./r-extras\"]\n",
        )
        .unwrap();

        std::env::set_var("HOME", &home);
        std::env::remove_var("USERPROFILE");
        std::env::remove_var("XDG_CONFIG_HOME");
        let names: Vec<String> = discover_from(&repo).into_iter().map(|s| s.name).collect();
        std::env::remove_var("HOME");
        assert!(names.contains(&"g_skill".to_string()), "{names:?}");
        assert!(names.contains(&"r_skill".to_string()), "{names:?}");
    }

    #[test]
    fn scaffold_creates_skill_and_registers_extra_dir_when_no_toml() {
        let tmp = TempDir::new().unwrap();
        let written = scaffold_in(tmp.path(), "pages", None).unwrap();
        assert!(written.ends_with("skill-data/pages/SKILL.md"));
        let body = fs::read_to_string(&written).unwrap();
        assert!(body.contains("name: pages"));
        assert!(body.contains("TODO"));
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[skills]"));
        assert!(toml.contains("./skill-data"));

        let names: Vec<String> = discover_from(tmp.path())
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"pages".to_string()), "{names:?}");
    }

    #[test]
    fn scaffold_refuses_to_overwrite_existing_skill() {
        let tmp = TempDir::new().unwrap();
        scaffold_in(tmp.path(), "pages", None).unwrap();
        let err = scaffold_in(tmp.path(), "pages", None).unwrap_err();
        assert!(err.to_string().contains("refusing to overwrite"));
    }

    #[test]
    fn scaffold_appends_to_existing_extra_dirs_array() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./pre-existing\"]\n",
        )
        .unwrap();
        scaffold_in(tmp.path(), "pages", Some("./skill-data")).unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("./pre-existing"), "lost prior entry: {toml}");
        assert!(toml.contains("./skill-data"), "missing new entry: {toml}");
    }

    #[test]
    fn scaffold_inserts_skills_section_into_plugins_only_toml() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[plugins]\nauth = \"/some/binary\"\n",
        )
        .unwrap();
        scaffold_in(tmp.path(), "glossary", None).unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        assert!(toml.contains("[plugins]"), "{toml}");
        assert!(toml.contains("[skills]"), "{toml}");
        assert!(toml.contains("./skill-data"), "{toml}");
    }

    #[test]
    fn scaffold_does_not_duplicate_existing_registration() {
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("agent-qa.toml"),
            "[skills]\nextra-dirs = [\"./skill-data\"]\n",
        )
        .unwrap();
        scaffold_in(tmp.path(), "pages", None).unwrap();
        let toml = fs::read_to_string(tmp.path().join("agent-qa.toml")).unwrap();
        let count = toml.matches("./skill-data").count();
        assert_eq!(count, 1, "duplicated entry: {toml}");
    }

    #[test]
    fn scaffold_rejects_invalid_names() {
        for bad in ["", "a/b", ".hidden", "-flag"] {
            assert!(scaffold(bad, None).is_err(), "accepted bad name {bad:?}");
        }
    }
}
