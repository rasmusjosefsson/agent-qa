//! `scenario` verb — operations on a scenario JSON document.
//!
//! Subverbs:
//!   scenario validate <file>      Validate against the embedded schema.
//!   scenario summary <file>       One-line per-step summary.
//!   scenario inputs <file>        List declared inputs (--json optional).
//!   scenario insert <file> ...    Splice a validated step into a saved scenario.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::scenario::{InputDecl, InputType, Locator, Scenario, Step, Value};
use crate::schema;

pub fn run(args: &[String]) -> Result<u8> {
    // `-h|--help` anywhere wins over positional parsing — otherwise a
    // file-taking subverb (`scenario lint --help`) reads "--help" as a path.
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        help();
        return Ok(0);
    }
    match args.first().map(String::as_str) {
        Some("validate") => {
            let file = args.get(1).ok_or_else(|| {
                anyhow!("usage: scenario validate <file> [--json | --format <fmt>]")
            })?;
            let mut format = if args.iter().any(|a| a == "--json") {
                LintFormat::Json
            } else {
                LintFormat::Text
            };
            let mut it = args.iter().peekable();
            while let Some(a) = it.next() {
                if a == "--format" {
                    let v = it
                        .next()
                        .cloned()
                        .ok_or_else(|| anyhow!("--format requires a value"))?;
                    format = parse_lint_format(&v)?;
                } else if let Some(v) = a.strip_prefix("--format=") {
                    format = parse_lint_format(v)?;
                }
            }
            validate(&scenario_file_arg(file), format)
        }
        Some("validate-all") => {
            let mut format = if args.iter().any(|a| a == "--json") {
                LintFormat::Json
            } else {
                LintFormat::Text
            };
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" => {}
                    "--root" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--root requires a directory"))?;
                        root = Some(PathBuf::from(v));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    "--format" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--format requires a value"))?;
                        format = parse_lint_format(&v)?;
                    }
                    s if s.starts_with("--format=") => {
                        format = parse_lint_format(&s["--format=".len()..])?;
                    }
                    other => {
                        bail!("validate-all: unknown argument {other:?} — usage: scenario validate-all [--root <dir>] [--json | --format text|json|github]")
                    }
                }
            }
            validate_all(format, root.as_deref())
        }
        Some("ls") => {
            let mut filter: Option<String> = None;
            let mut json_out = false;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" => json_out = true,
                    "--filter" => {
                        filter = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                        )
                    }
                    s if s.starts_with("--filter=") => {
                        filter = Some(s["--filter=".len()..].to_string())
                    }
                    "--root" => {
                        root = Some(PathBuf::from(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--root requires a directory"))?,
                        ));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            ls(filter.as_deref(), json_out, root.as_deref())
        }
        Some("latest") => {
            let mut filter: Option<String> = None;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--filter" => {
                        filter = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                        )
                    }
                    s if s.starts_with("--filter=") => {
                        filter = Some(s["--filter=".len()..].to_string())
                    }
                    "--root" => {
                        root = Some(PathBuf::from(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--root requires a directory"))?,
                        ));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            latest(filter.as_deref(), root.as_deref())
        }
        Some("count") => {
            let mut filter: Option<String> = None;
            let mut json_out = false;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" => json_out = true,
                    "--filter" => {
                        filter = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                        )
                    }
                    s if s.starts_with("--filter=") => {
                        filter = Some(s["--filter=".len()..].to_string())
                    }
                    "--root" => {
                        root = Some(PathBuf::from(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--root requires a directory"))?,
                        ));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            count(filter.as_deref(), json_out, root.as_deref())
        }
        Some("diff") => {
            let a = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario diff <a> <b>"))?;
            let b = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: scenario diff <a> <b>"))?;
            diff(Path::new(a), Path::new(b))
        }
        Some("hash") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario hash <file>"))?;
            hash(&scenario_file_arg(file))
        }
        Some("id") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario id <file>"))?;
            id(&scenario_file_arg(file))
        }
        Some("intent") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario intent <file>"))?;
            intent(&scenario_file_arg(file))
        }
        Some("step-ids") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario step-ids <file>"))?;
            step_ids(&scenario_file_arg(file))
        }
        Some("field") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario field <file> <name>"))?;
            let name = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: scenario field <file> <name>"))?;
            field(&scenario_file_arg(file), name)
        }
        Some("coverage") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario coverage <file|sid> [--json]"))?;
            let json_out = args.iter().any(|a| a == "--json");
            coverage(&scenario_file_arg(file), json_out)
        }
        Some("coverage-all") => {
            let mut filter: Option<String> = None;
            let mut json_out = false;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" => json_out = true,
                    "--filter" => {
                        filter = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                        )
                    }
                    s if s.starts_with("--filter=") => {
                        filter = Some(s["--filter=".len()..].to_string())
                    }
                    "--root" => {
                        root = Some(PathBuf::from(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--root requires a directory"))?,
                        ));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            coverage_all(filter.as_deref(), json_out, root.as_deref())
        }
        Some("check") => {
            let file = args.get(1).ok_or_else(|| {
                anyhow!("usage: scenario check <file> [--strict] [--format text|json|github]")
            })?;
            let strict = args.iter().any(|a| a == "--strict");
            let mut format = LintFormat::Text;
            let mut it = args.iter().peekable();
            while let Some(a) = it.next() {
                if a == "--format" {
                    let v = it
                        .next()
                        .cloned()
                        .ok_or_else(|| anyhow!("--format requires a value"))?;
                    format = parse_lint_format(&v)?;
                } else if let Some(v) = a.strip_prefix("--format=") {
                    format = parse_lint_format(v)?;
                }
            }
            check(&scenario_file_arg(file), strict, format)
        }
        Some("check-all") => {
            let strict = args.iter().any(|a| a == "--strict");
            let mut format = LintFormat::Text;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--strict" => {}
                    "--root" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--root requires a directory"))?;
                        root = Some(PathBuf::from(v));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    "--format" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--format requires a value"))?;
                        format = parse_lint_format(&v)?;
                    }
                    s if s.starts_with("--format=") => {
                        format = parse_lint_format(&s["--format=".len()..])?;
                    }
                    other => {
                        bail!("check-all: unknown argument {other:?} — usage: scenario check-all [--strict] [--root <dir>] [--format text|json|github]")
                    }
                }
            }
            check_all(strict, format, root.as_deref())
        }
        Some("lint") => {
            if args.iter().any(|a| a == "--list-rules") {
                return list_lint_rules(args.iter().any(|a| a == "--json"));
            }
            let file = args.get(1).ok_or_else(|| {
                anyhow!("usage: scenario lint <file> [--json] [--strict] [--rule <code>] [--format github]")
            })?;
            let json_out = args.iter().any(|a| a == "--json");
            let strict = args.iter().any(|a| a == "--strict");
            let mut only_rules: Vec<String> = Vec::new();
            let mut exclude_rules: Vec<String> = Vec::new();
            let mut format: LintFormat = if json_out {
                LintFormat::Json
            } else {
                LintFormat::Text
            };
            let mut it = args.iter().skip(2).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--rule" => {
                        only_rules.push(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--rule requires a code"))?,
                        )
                    }
                    s if s.starts_with("--rule=") => {
                        only_rules.push(s["--rule=".len()..].to_string())
                    }
                    "--exclude-rule" => exclude_rules.push(
                        it.next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--exclude-rule requires a code"))?,
                    ),
                    s if s.starts_with("--exclude-rule=") => {
                        exclude_rules.push(s["--exclude-rule=".len()..].to_string())
                    }
                    "--format" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--format requires a value"))?;
                        format = parse_lint_format(&v)?;
                    }
                    s if s.starts_with("--format=") => {
                        format = parse_lint_format(&s["--format=".len()..])?;
                    }
                    _ => {}
                }
            }
            let rules = if only_rules.is_empty() {
                None
            } else {
                Some(only_rules)
            };
            let excl = if exclude_rules.is_empty() {
                None
            } else {
                Some(exclude_rules)
            };
            lint(
                &scenario_file_arg(file),
                format,
                strict,
                rules.as_deref(),
                excl.as_deref(),
            )
        }
        Some("lint-all") => {
            let json_out = args.iter().any(|a| a == "--json");
            let strict = args.iter().any(|a| a == "--strict");
            let mut only_rules: Vec<String> = Vec::new();
            let mut exclude_rules: Vec<String> = Vec::new();
            let mut lint_root: Option<PathBuf> = None;
            let mut format: LintFormat = if json_out {
                LintFormat::Json
            } else {
                LintFormat::Text
            };
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" | "--strict" => {}
                    "--root" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--root requires a directory"))?;
                        lint_root = Some(PathBuf::from(v));
                    }
                    s if s.starts_with("--root=") => {
                        lint_root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    "--rule" => {
                        only_rules.push(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--rule requires a code"))?,
                        )
                    }
                    s if s.starts_with("--rule=") => {
                        only_rules.push(s["--rule=".len()..].to_string())
                    }
                    "--exclude-rule" => exclude_rules.push(
                        it.next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--exclude-rule requires a code"))?,
                    ),
                    s if s.starts_with("--exclude-rule=") => {
                        exclude_rules.push(s["--exclude-rule=".len()..].to_string())
                    }
                    "--format" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--format requires a value"))?;
                        format = parse_lint_format(&v)?;
                    }
                    s if s.starts_with("--format=") => {
                        format = parse_lint_format(&s["--format=".len()..])?;
                    }
                    other => {
                        bail!("lint-all: unknown argument {other:?} — usage: scenario lint-all [--strict] [--root <dir>] [--rule <code>] [--exclude-rule <code>] [--format text|json|github]")
                    }
                }
            }
            let rules = if only_rules.is_empty() {
                None
            } else {
                Some(only_rules)
            };
            let excl = if exclude_rules.is_empty() {
                None
            } else {
                Some(exclude_rules)
            };
            lint_all(
                format,
                strict,
                rules.as_deref(),
                excl.as_deref(),
                lint_root.as_deref(),
            )
        }
        Some("rename") => {
            let from = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario rename <sid> <new-sid>"))?;
            let to = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: scenario rename <sid> <new-sid>"))?;
            rename(from, to)
        }
        Some("copy") => {
            let from = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario copy <sid> <new-sid>"))?;
            let to = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: scenario copy <sid> <new-sid>"))?;
            copy(from, to)
        }
        Some("extract") => {
            let sid = args.get(1).ok_or_else(|| {
                anyhow!(
                    "usage: scenario extract <sid> [--run <runId|latest>] [--through <stepId>] [--to <new-sid>] [--json]"
                )
            })?;
            let mut run: Option<String> = None;
            let mut through: Option<String> = None;
            let mut to: Option<String> = None;
            let mut json = false;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--run" => {
                        run = Some(args.get(i + 1).cloned().ok_or_else(|| anyhow!("--run expects a run id"))?);
                        i += 1;
                    }
                    "--through" => {
                        through = Some(
                            args.get(i + 1).cloned().ok_or_else(|| anyhow!("--through expects a step id"))?,
                        );
                        i += 1;
                    }
                    "--to" => {
                        to = Some(args.get(i + 1).cloned().ok_or_else(|| anyhow!("--to expects a new sid"))?);
                        i += 1;
                    }
                    "--json" => json = true,
                    s if s.starts_with("--run=") => run = Some(s["--run=".len()..].to_string()),
                    s if s.starts_with("--through=") => {
                        through = Some(s["--through=".len()..].to_string())
                    }
                    s if s.starts_with("--to=") => to = Some(s["--to=".len()..].to_string()),
                    other => bail!("unexpected arg {other:?}; usage: scenario extract <sid> [--run <runId|latest>] [--through <stepId>] [--to <new-sid>] [--json]"),
                }
                i += 1;
            }
            extract(sid, run.as_deref(), through.as_deref(), to.as_deref(), json)
        }
        Some("tag") => {
            let sid = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario tag <sid> [--add <a,b>] [--remove <c,d>] [--json]"))?;
            let mut add: Vec<String> = Vec::new();
            let mut remove: Vec<String> = Vec::new();
            let mut json = false;
            let mut it = args.iter().skip(2);
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--add" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--add requires a comma-separated value"))?;
                        add.extend(v.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
                    }
                    s if s.starts_with("--add=") => {
                        add.extend(s["--add=".len()..].split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
                    }
                    "--remove" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--remove requires a comma-separated value"))?;
                        remove.extend(v.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
                    }
                    s if s.starts_with("--remove=") => {
                        remove.extend(s["--remove=".len()..].split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()));
                    }
                    "--json" => json = true,
                    other => bail!("unexpected arg {other:?}; usage: scenario tag <sid> [--add <a,b>] [--remove <c,d>] [--json]"),
                }
            }
            tag(sid, &add, &remove, json)
        }
        Some("delete") => {
            let sid = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario delete <sid> --yes"))?;
            let confirmed = args.iter().any(|a| a == "--yes" || a == "-y");
            delete(sid, confirmed)
        }
        Some("prune-replays") => {
            let sid = args.get(1).ok_or_else(|| {
                anyhow!("usage: scenario prune-replays <sid> --keep N [--yes] [--keep-failed]")
            })?;
            let mut keep: Option<usize> = None;
            let mut confirmed = false;
            let mut keep_failed = false;
            let mut it = args.iter().skip(2);
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--yes" | "-y" => confirmed = true,
                    "--keep-failed" => keep_failed = true,
                    "--keep" => {
                        let v = it
                            .next()
                            .ok_or_else(|| anyhow!("--keep requires a non-negative integer"))?;
                        keep = Some(v.parse().map_err(|_| {
                            anyhow!("--keep expects a non-negative integer, got {v:?}")
                        })?);
                    }
                    s if s.starts_with("--keep=") => {
                        let v = &s["--keep=".len()..];
                        keep = Some(v.parse().map_err(|_| {
                            anyhow!("--keep expects a non-negative integer, got {v:?}")
                        })?);
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            let keep = keep.ok_or_else(|| anyhow!("--keep <N> is required"))?;
            prune_replays(sid, keep, confirmed, keep_failed)
        }
        Some("prune-all") => {
            let mut keep: Option<usize> = None;
            let mut confirmed = false;
            let mut keep_failed = false;
            let mut root: Option<PathBuf> = None;
            let mut it = args.iter().skip(1).peekable();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--yes" | "-y" => confirmed = true,
                    "--keep-failed" => keep_failed = true,
                    "--keep" => {
                        let v = it
                            .next()
                            .ok_or_else(|| anyhow!("--keep requires a non-negative integer"))?;
                        keep = Some(v.parse().map_err(|_| {
                            anyhow!("--keep expects a non-negative integer, got {v:?}")
                        })?);
                    }
                    s if s.starts_with("--keep=") => {
                        let v = &s["--keep=".len()..];
                        keep = Some(v.parse().map_err(|_| {
                            anyhow!("--keep expects a non-negative integer, got {v:?}")
                        })?);
                    }
                    "--root" => {
                        root = Some(PathBuf::from(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--root requires a directory"))?,
                        ));
                    }
                    s if s.starts_with("--root=") => {
                        root = Some(PathBuf::from(&s["--root=".len()..]));
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            let keep = keep.ok_or_else(|| anyhow!("--keep <N> is required"))?;
            prune_all(keep, confirmed, keep_failed, root.as_deref())
        }
        Some("summary") => {
            let file = args.get(1).ok_or_else(|| {
                anyhow!("usage: scenario summary <file> [--filter <pattern>] [--json]")
            })?;
            let mut filter: Option<String> = None;
            let mut json_out = false;
            let mut it = args.iter().skip(2);
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--json" => json_out = true,
                    "--filter" => {
                        filter = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--filter requires a substring"))?,
                        )
                    }
                    s if s.starts_with("--filter=") => {
                        filter = Some(s["--filter=".len()..].to_string())
                    }
                    other => bail!("unknown flag {other:?}"),
                }
            }
            summary(&scenario_file_arg(file), filter.as_deref(), json_out)
        }
        Some("inputs") => {
            let file = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: scenario inputs <file> [--json]"))?;
            let json_out = args.iter().any(|a| a == "--json");
            inputs(&scenario_file_arg(file), json_out)
        }
        Some("redact") => {
            let mut file: Option<&str> = None;
            let mut name: Option<String> = None;
            let mut value: Option<String> = None;
            let mut selector: Option<String> = None;
            let mut step: Option<String> = None;
            let mut dry_run = false;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--dry-run" => dry_run = true,
                    "--name" => {
                        name = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--name requires an identifier"))?,
                        )
                    }
                    s if s.starts_with("--name=") => {
                        name = Some(s["--name=".len()..].to_string())
                    }
                    "--value" => {
                        value = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--value requires the literal to redact"))?,
                        )
                    }
                    s if s.starts_with("--value=") => {
                        value = Some(s["--value=".len()..].to_string())
                    }
                    "--selector" => {
                        selector = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--selector requires a css selector"))?,
                        )
                    }
                    s if s.starts_with("--selector=") => {
                        selector = Some(s["--selector=".len()..].to_string())
                    }
                    "--step" => {
                        step = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--step requires a step id"))?,
                        )
                    }
                    s if s.starts_with("--step=") => {
                        step = Some(s["--step=".len()..].to_string())
                    }
                    other if other.starts_with("--") => bail!("unknown flag {other:?}"),
                    other => {
                        if file.is_none() {
                            file = Some(other);
                        } else {
                            bail!("unexpected positional {other:?}");
                        }
                    }
                }
            }
            let file = file.ok_or_else(|| {
                anyhow!(
                    "usage: scenario redact <file> --name <VAR> (--value <literal> | --selector <css> | --step <stepId>) [--dry-run]"
                )
            })?;
            let name = name.ok_or_else(|| anyhow!("--name is required"))?;
            redact(
                Path::new(file),
                &name,
                value.as_deref(),
                selector.as_deref(),
                step.as_deref(),
                dry_run,
            )
        }
        Some("insert") => {
            let mut file: Option<&str> = None;
            let mut kind: Option<&str> = None;
            let mut draft: Option<&str> = None;
            let mut after: Option<String> = None;
            let mut at: Option<usize> = None;
            let mut it = args[1..].iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--after" => {
                        after = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--after requires a step id"))?,
                        )
                    }
                    s if s.starts_with("--after=") => {
                        after = Some(s["--after=".len()..].to_string())
                    }
                    "--at" => {
                        let v = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--at requires an index"))?;
                        at = Some(
                            v.parse()
                                .map_err(|_| anyhow!("--at must be a non-negative integer"))?,
                        )
                    }
                    s if s.starts_with("--at=") => {
                        at = Some(
                            s["--at=".len()..]
                                .parse()
                                .map_err(|_| anyhow!("--at must be a non-negative integer"))?,
                        )
                    }
                    other if other.starts_with("--") => bail!("unknown flag {other:?}"),
                    other => {
                        if file.is_none() {
                            file = Some(other);
                        } else if kind.is_none() {
                            kind = Some(other);
                        } else if draft.is_none() {
                            draft = Some(other);
                        } else {
                            bail!("unexpected positional {other:?}");
                        }
                    }
                }
            }
            let (file, kind, draft) = match (file, kind, draft) {
                (Some(f), Some(k), Some(d)) => (f, k, d),
                _ => bail!(
                    "usage: scenario insert <file> <do|check> <draft-json> [--after <stepId> | --at <index>]"
                ),
            };
            insert(&scenario_file_arg(file), kind, draft, after.as_deref(), at)
        }
        Some("new") => {
            let mut file: Option<&str> = None;
            let mut force = false;
            let mut url = "https://example.com/".to_string();
            let mut intent = "describe what this scenario does".to_string();
            let mut from_har: Option<String> = None;
            let rest = &args[1..];
            let mut it = rest.iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--force" => force = true,
                    "--from-har" => {
                        from_har = Some(
                            it.next()
                                .cloned()
                                .ok_or_else(|| anyhow!("--from-har requires a value"))?,
                        )
                    }
                    s if s.starts_with("--from-har=") => {
                        from_har = Some(s["--from-har=".len()..].to_string())
                    }
                    "--url" => {
                        url = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--url requires a value"))?
                    }
                    s if s.starts_with("--url=") => url = s["--url=".len()..].to_string(),
                    "--intent" => {
                        intent = it
                            .next()
                            .cloned()
                            .ok_or_else(|| anyhow!("--intent requires a value"))?
                    }
                    s if s.starts_with("--intent=") => intent = s["--intent=".len()..].to_string(),
                    other if other.starts_with("--") => bail!("unknown flag {other:?}"),
                    other => {
                        if file.is_some() {
                            bail!("unexpected positional {other:?}; usage: scenario new <file>");
                        }
                        file = Some(other);
                    }
                }
            }
            let file = file.ok_or_else(|| anyhow!("usage: scenario new <file>"))?;
            if let Some(har) = from_har {
                return new_from_har(Path::new(file), Path::new(&har), force, &intent);
            }
            new(Path::new(file), force, &url, &intent)
        }
        Some("--help" | "-h" | "help") | None => {
            help();
            Ok(0)
        }
        Some(other) => bail!(
            "unknown subverb {other:?}. Try: validate <file> | summary <file> | inputs <file> [--json] | new <file> | insert <file> <do|check> <draft-json>"
        ),
    }
}

fn help() {
    println!(
        "agent-qa scenario — operations on a scenario JSON\n\nUsage:\n  agent-qa scenario validate <file> [--json | --format text|json|github]\n                                            Schema-validate the scenario (use '-' for stdin)\n  agent-qa scenario check <file> [--strict]  Schema-validate AND lint in one pass\n                                            (combined exit code: 0 iff both pass).\n                                            (use '-' for stdin)\n  agent-qa scenario check-all [--strict] [--root <dir>] [--format text|json|github]\n                                            Same combo across every scenario under the\n                                            scenarios root (--root overrides it);\n                                            exit 1 iff any fail.\n  agent-qa scenario validate-all [--root <dir>] [--json | --format text|json|github]\n                                            Schema-validate every scenario under the\n                                            scenarios root (--root overrides it);\n                                            exit 1 iff any fail\n  agent-qa scenario ls [--filter <substr>] [--root <dir>] [--json]\n                                            Print every sid under the scenarios root\n                                            (--root overrides it),\n                                            one per line (lex sort).\n  agent-qa scenario latest [--filter <substr>] [--root <dir>]\n                                            Print the sid whose scenario.json was most\n                                            recently modified (--root overrides the root).\n  agent-qa scenario count [--filter <substr>] [--root <dir>] [--json]\n                                            Print the number of scenarios under the root\n                                            (--root overrides it).\n                                            --filter narrows to sids containing <substr>.\n                                            --json wraps the count + filter in a JSON object.\n  agent-qa scenario summary  <file> [--filter <substr>] [--json]\n                                            Per-step summary (id, kind, verb/claim).\n                                            (use '-' for stdin)\n                                            --filter: case-insensitive substring\n                                            matched against id/intent/verb.\n  agent-qa scenario inputs   <file> [--json] List declared inputs (type/default/sensitive)\n  agent-qa scenario new      <file>          Scaffold a minimal valid scenario.json\n                                            (--force to overwrite, --url, --intent,\n                                            --from-har <har> rebuilds a nav skeleton\n                                            from a captured HAR — goto + url check +\n                                            idle wait per document nav; pair with\n                                            --mock-from/--offline at replay)\n  agent-qa scenario insert <file> <do|check> <draft-json> [--after <stepId> | --at <index>]
                                            Splice a validated step into a saved scenario.
                                            Draft shape matches record-step (id/kind
                                            omitted); position defaults to the end.
  agent-qa scenario redact <file> --name <VAR> (--value <literal> | --selector <css> | --step <stepId>) [--dry-run]
                                            Replace a recorded literal (password, token) with a
                                            sensitive inputs.<VAR> reference so the file is safe to
                                            commit. --value sweeps every step's strings; --selector /
                                            --step rewrite a do-step's value without the secret on
                                            the command line.
  agent-qa scenario diff <a> <b>             Unified diff between two scenario.json files\n                                            (canonicalised JSON; exit 1 on difference)\n  agent-qa scenario hash <file>              SHA-256 of scenario.json bytes (same algorithm\n                                            replay + heal-promote use for the rebase guard)\n  agent-qa scenario id <file>                Print the scenario's id field (one line)\n  agent-qa scenario intent <file>            Print the scenario's intent field (one line)\n  agent-qa scenario step-ids <file>          Print every step id, one per line\n  agent-qa scenario field <file> <name>      Print any top-level scenario field (id, intent,\n                                            schema, etc.); object/array → compact JSON.\n  agent-qa scenario coverage <file> [--json] Per-step check coverage: how many do steps are\n                                            followed by a check claim, and how many are bare.\n  agent-qa scenario coverage-all [--filter <substr>] [--root <dir>] [--json]\n                                            The same ratio rolled up across every scenario under\n                                            the root (--root overrides it) — per-scenario rows sorted worst-first plus\n                                            an OVERALL rollup.\n  agent-qa scenario lint <file> [--json] [--strict] [--rule <code>]* [--exclude-rule <code>]* [--format text|json|github]\n                                            Run common lints (use '-' for stdin)\n                                            (duplicate ids, empty intent, bare do,\n                                            undeclared/unused inputs). Exit 1 iff\n                                            any errors are reported (--strict treats\n                                            warnings as errors). --rule narrows to specific\n                                            codes; --exclude-rule subtracts (both repeatable).\n                                            --format github emits GitHub Actions annotations.\n  agent-qa scenario lint --list-rules [--json]\n                                            Enumerate the lint rules + their severities.\n  agent-qa scenario lint-all [--json] [--strict] [--root <dir>] [--rule <code>]* [--exclude-rule <code>]* [--format text|json|github]\n                                            Run lints against every scenario under the\n                                            scenarios root (--root overrides it);\n                                            exit 1 iff any errors are reported\n                                            (--strict treats warnings as errors). --rule\n                                            narrows to specific codes; --exclude-rule\n                                            subtracts (both repeatable).\n  agent-qa scenario rename <sid> <new-sid>   Rename a scenario: patches scenario.json's id\n                                            field, then moves the directory under the\n                                            scenarios root. (refuses to overwrite).\n  agent-qa scenario copy <sid> <new-sid>     Copy a scenario (scenario.json with id patched +\n                                            baselines/, files/, inputs.local.json carried;\n                                            replays NOT copied). Refuses to overwrite.\n  agent-qa scenario delete <sid> [--yes/-y]  Remove a scenario directory + all its replays.\n                                            Dry-run by default; --yes confirms.\n  agent-qa scenario prune-replays <sid> --keep N [--yes] [--keep-failed]\n                                            Keep the N most recent replays under\n                                            <sid>/replays/; dry-run by default.\n                                            --keep-failed preserves all non-zero-exit\n                                            runs regardless of N.\n  agent-qa scenario prune-all --keep N [--yes] [--keep-failed] [--root <dir>]\n                                            Like prune-replays but across every scenario\n                                            under the scenarios root (--root overrides it). --keep-failed\n                                            preserves failed runs per scenario."
    );
}

fn new(path: &Path, force: bool, url: &str, intent: &str) -> Result<u8> {
    if path.exists() && !force {
        bail!(
            "refusing to overwrite {} (pass --force to replace)",
            path.display()
        );
    }
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scenario")
        .to_string();
    let body = serde_json::json!({
        "schema": "scenario/2",
        "id": id,
        "intent": intent,
        "env": {
            "open": [
                { "kind": "nav", "url": url, "intent": "land on the page" }
            ]
        },
        "steps": [
            {
                "id": "s1",
                "intent": "url is set",
                "kind": "check",
                "claim": {
                    "subject": { "url": true },
                    "predicate": "exists"
                }
            }
        ]
    });
    schema::validate_value(&body).context("scaffolded scenario failed schema validation")?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
        }
    }
    let mut bytes = serde_json::to_string_pretty(&body)?.into_bytes();
    bytes.push(b'\n');
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    println!("wrote {} ({} bytes)", path.display(), bytes.len());
    Ok(0)
}

/// `scenario new <file> --from-har <har>` — reconstruct a nav skeleton
/// from a captured HAR: one goto + url-check + network-idle wait per
/// document navigation (entries grouped by `pageref`, or the HTML
/// responses in order when the export has none). Pair with
/// `replay --mock-from <har> --offline` to re-run the captured session
/// against its own backend state.
fn new_from_har(path: &Path, har_path: &Path, force: bool, intent: &str) -> Result<u8> {
    if path.exists() && !force {
        bail!(
            "refusing to overwrite {} (pass --force to replace)",
            path.display()
        );
    }
    let body =
        fs::read_to_string(har_path).with_context(|| format!("read {}", har_path.display()))?;
    let har: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("unparseable HAR {}", har_path.display()))?;
    let entries = har["log"]["entries"]
        .as_array()
        .ok_or_else(|| anyhow!("{}: no log.entries", har_path.display()))?;
    if entries.is_empty() {
        bail!("{}: empty log.entries", har_path.display());
    }

    // Document navigations, in capture order: GET entries whose response
    // is HTML. When `pageref`s exist, keep at most the first doc per page
    // (the page's own navigation, not an html fragment it fetched).
    let mut navs: Vec<String> = Vec::new();
    let mut seen_pages: Vec<String> = Vec::new();
    let mut docs = 0usize;
    for e in entries {
        let method = e["request"]["method"].as_str().unwrap_or("");
        let url = e["request"]["url"].as_str().unwrap_or("");
        let mime = e["response"]["content"]["mimeType"].as_str().unwrap_or("");
        if method != "GET" || !mime.contains("html") || url.is_empty() {
            continue;
        }
        docs += 1;
        if let Some(page) = e["pageref"].as_str() {
            if seen_pages.iter().any(|p| p == page) {
                continue;
            }
            seen_pages.push(page.to_string());
        }
        if navs.last().map(|u| u.as_str()) == Some(url) {
            continue; // consecutive reload of the same document
        }
        navs.push(url.to_string());
    }
    if navs.is_empty() {
        bail!(
            "{}: no HTML document navigations in the HAR ({} entries{} — it may start mid-session; `--mock-from` still replays its API traffic)",
            har_path.display(),
            entries.len(),
            if docs > 0 { ", docs were fetched-only" } else { "" }
        );
    }

    let mut steps: Vec<serde_json::Value> = Vec::new();
    let mut n = 0usize;
    for url in &navs {
        n += 1;
        steps.push(serde_json::json!({
            "id": format!("s{n}"),
            "intent": format!("navigate to {url}"),
            "kind": "do",
            "verb": "goto",
            "value": { "from": "literal", "literal": url }
        }));
        n += 1;
        let label = url
            .split_once("://")
            .map(|(_, rest)| rest)
            .unwrap_or(url)
            .split(['?', '#'])
            .next()
            .unwrap_or(url);
        steps.push(serde_json::json!({
            "id": format!("s{n}"),
            "intent": format!("landed on {label}"),
            "kind": "check",
            "claim": { "subject": { "url": true }, "predicate": "exists" }
        }));
        n += 1;
        steps.push(serde_json::json!({
            "id": format!("s{n}"),
            "intent": "page settles (network idle)",
            "kind": "do",
            "verb": "wait",
            "params": { "idle": true, "timeoutMs": 10_000 }
        }));
    }

    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("scenario")
        .to_string();
    let intent = if intent.is_empty() {
        format!(
            "reconstructed from {}",
            har_path.file_name().unwrap_or_default().to_string_lossy()
        )
    } else {
        intent.to_string()
    };
    let body = serde_json::json!({
        "schema": "scenario/2",
        "id": id,
        "intent": intent,
        "env": {
            "open": [
                { "kind": "nav", "url": navs[0], "intent": "land on the page" }
            ]
        },
        "steps": steps,
    });
    schema::validate_value(&body).context("from-har scenario failed schema validation")?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
        }
    }
    let mut bytes = serde_json::to_string_pretty(&body)?.into_bytes();
    bytes.push(b'\n');
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    println!(
        "wrote {} ({} bytes) — {} navigation(s); replay it against the captured backend with `replay <sid> --mock-from {} --offline`",
        path.display(),
        bytes.len(),
        navs.len(),
        har_path.display()
    );
    Ok(0)
}

/// `scenario insert <file> <do|check> <draft-json> [--after <stepId> | --at
/// <index>]` splices a validated step into a saved scenario. The draft uses
/// the same shape as `record-step` (id/kind omitted — kind comes from the
/// arg, id is minted as the first free `s<n>`). Position defaults to the
/// end; `--after` resolves a step id to the slot right behind it, `--at`
/// takes a raw index.
fn insert(
    path: &Path,
    kind_arg: &str,
    draft_json: &str,
    after: Option<&str>,
    at: Option<usize>,
) -> Result<u8> {
    if after.is_some() && at.is_some() {
        bail!("pass --after or --at, not both");
    }
    let mut sc = load_scenario(path)?;
    // Editing a scenario a live recording has loaded silently loses this
    // splice at the next flush — the buffer copy wins.
    if let Some(steps) = bound_recording_steps(&sc.id)? {
        eprintln!(
            "warning: {:?} is bound to the active recording ({steps} step(s)) — `buffer load` again after editing, or the next flush will overwrite this change",
            sc.id
        );
    }
    let kind = crate::record_step::StepKind::parse(kind_arg)?;
    let pos = match (after, at) {
        (Some(id), None) => {
            let idx = sc
                .steps
                .iter()
                .position(|s| s.id() == id)
                .ok_or_else(|| anyhow!("no step with id {id:?}"))?;
            idx + 1
        }
        (None, Some(i)) => {
            if i > sc.steps.len() {
                bail!("--at {i} out of range ({} steps)", sc.steps.len());
            }
            i
        }
        (None, None) => sc.steps.len(),
        (Some(_), Some(_)) => unreachable!(),
    };
    let mut n = sc.steps.len();
    let step_id = loop {
        let candidate = format!("s{n}");
        if sc.steps.iter().all(|s| s.id() != candidate) {
            break candidate;
        }
        n += 1;
    };
    let payload: serde_json::Value =
        serde_json::from_str(draft_json).context("draft must be a JSON object")?;
    let step = crate::record_step::parse_draft(kind, &payload, &step_id)?;
    sc.steps.insert(pos, step);
    let body = serde_json::to_value(&sc)?;
    schema::validate_value(&body)
        .context("scenario with the inserted step failed schema validation")?;
    let mut bytes = serde_json::to_string_pretty(&body)?.into_bytes();
    bytes.push(b'\n');
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    println!(
        "inserted {kind_arg} step at {pos} (id={step_id}); {} step(s)",
        sc.steps.len()
    );
    Ok(0)
}

/// Recursively rewrite occurrences of `needle` inside every string in `v` to
/// `token` (`{{vars.NAME}}`). Returns the number of strings touched.
fn replace_in_json(v: &mut serde_json::Value, needle: &str, token: &str) -> usize {
    match v {
        serde_json::Value::String(s) => {
            if s.contains(needle) {
                *s = s.replace(needle, token);
                1
            } else {
                0
            }
        }
        serde_json::Value::Array(items) => items
            .iter_mut()
            .map(|i| replace_in_json(i, needle, token))
            .sum(),
        serde_json::Value::Object(map) => map
            .values_mut()
            .map(|i| replace_in_json(i, needle, token))
            .sum(),
        _ => 0,
    }
}

/// `scenario redact` — pull a recorded literal (a typed password, a token)
/// out of the committed scenario.json and replace it with a declared
/// `inputs.<name>` (sensitive) reference, so the file is safe to commit.
///
/// Three ways to find the secret carrier:
///   --value <literal>   sweep every step for strings containing the
///                       literal; whole `value.literal` matches upgrade to
///                       `{from: "input"}` refs, substring occurrences get a
///                       `{{vars.NAME}}` token (also covers claim values,
///                       params, templates).
///   --selector <css>    rewrite the `value` of every do-step whose raw css
///                       locator matches — no secret on the command line.
///   --step <stepId>     same, addressed by step id.
///
/// Both reference channels resolve at replay: `{"from": "input"}` reads
/// `inputs.<name>` and `{{vars.NAME}}` substitutes it too.
fn redact(
    path: &Path,
    name: &str,
    value: Option<&str>,
    selector: Option<&str>,
    step_id: Option<&str>,
    dry_run: bool,
) -> Result<u8> {
    if [value.is_some(), selector.is_some(), step_id.is_some()]
        .iter()
        .filter(|b| **b)
        .count()
        != 1
    {
        bail!("pass exactly one of --value / --selector / --step");
    }
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        bail!("--name {name:?} must be an identifier ([A-Za-z_][A-Za-z0-9_]*)");
    }
    if let Some(v) = value {
        if v.is_empty() {
            bail!("--value must not be empty");
        }
    }
    let token = format!("{{{{vars.{name}}}}}");
    let mut sc = load_scenario(path)?;
    if !dry_run {
        if let Some(steps) = bound_recording_steps(&sc.id)? {
            eprintln!(
                "warning: {:?} is bound to the active recording ({steps} step(s)) — `buffer load` again after editing, or the next flush will overwrite this change",
                sc.id
            );
        }
    }
    let mut upgraded = 0usize;
    let mut swept = 0usize;
    for step in sc.steps.iter_mut() {
        match step {
            Step::Do {
                id,
                on,
                value: step_value,
                ..
            } => {
                let targeted = if let Some(want) = step_id {
                    id == want
                } else if let Some(want) = selector {
                    matches!(
                        on,
                        Some(Locator::Raw(raw)) if raw.raw.value == want
                    )
                } else {
                    false
                };
                if targeted {
                    match step_value {
                        Some(Value::Literal { literal }) if literal.is_string() => {
                            *step_value = Some(Value::Input {
                                input: name.to_string(),
                            });
                            upgraded += 1;
                        }
                        Some(Value::Literal { .. }) => {
                            bail!("step {id}: value literal is not a string — edit it by hand")
                        }
                        _ => bail!("step {id}: no literal value to redact"),
                    }
                    continue;
                }
                if let Some(secret) = value {
                    // Whole-literal match upgrades to the typed input channel…
                    if let Some(Value::Literal {
                        literal: serde_json::Value::String(s),
                    }) = step_value
                    {
                        if *s == secret {
                            *step_value = Some(Value::Input {
                                input: name.to_string(),
                            });
                            upgraded += 1;
                            continue;
                        }
                    }
                    // …otherwise substring-sweep the whole step (params too).
                    let mut j = serde_json::to_value(&*step)?;
                    let n = replace_in_json(&mut j, secret, &token);
                    if n > 0 {
                        *step = serde_json::from_value(j)
                            .context("step failed to re-parse after redaction")?;
                        swept += n;
                    }
                }
            }
            Step::Check { .. } => {
                if let Some(secret) = value {
                    let mut j = serde_json::to_value(&*step)?;
                    let n = replace_in_json(&mut j, secret, &token);
                    if n > 0 {
                        *step = serde_json::from_value(j)
                            .context("step failed to re-parse after redaction")?;
                        swept += n;
                    }
                }
            }
        }
    }
    if upgraded + swept == 0 {
        bail!("nothing matched — no step carries that value/selector/step id");
    }
    let inputs = sc
        .inputs
        .get_or_insert_with(std::collections::BTreeMap::new);
    match inputs.get(name) {
        Some(existing) if existing.sensitive == Some(true) => {}
        Some(_) => bail!("inputs.{name} is already declared non-sensitive — pick another name"),
        None => {
            inputs.insert(
                name.to_string(),
                InputDecl {
                    ty: InputType::String,
                    default: None,
                    sensitive: Some(true),
                    items: None,
                    properties: None,
                    description: None,
                },
            );
        }
    }
    let body = serde_json::to_value(&sc)?;
    schema::validate_value(&body).context("redacted scenario failed schema validation")?;
    if dry_run {
        println!(
            "dry-run: would rewrite {upgraded} step value(s) to {{from: \"input\", input: \"{name}\"}} \
             and sweep {swept} string(s) to {token}; adds inputs.{name} (sensitive)"
        );
        return Ok(0);
    }
    let mut bytes = serde_json::to_string_pretty(&body)?.into_bytes();
    bytes.push(b'\n');
    fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    println!(
        "redacted {upgraded} step value(s) → input:{name} + {swept} embedded occurrence(s) → {token}; \
         declared inputs.{name} (sensitive). Supply it at replay: replay --input {name}=…"
    );
    Ok(0)
}

fn copy(from_sid: &str, to_sid: &str) -> Result<u8> {
    if from_sid == to_sid {
        bail!("scenario copy: from and to are the same ({from_sid:?})");
    }
    if to_sid.is_empty() || to_sid.contains('/') || to_sid.contains('\\') || to_sid.starts_with('.')
    {
        bail!(
            "scenario copy: new sid {to_sid:?} must be a non-empty, slash-free, non-dotfile name"
        );
    }
    let from_dir = crate::paths::scenario_dir(from_sid)?;
    if !from_dir.is_dir() {
        bail!("scenario copy: source not found at {}", from_dir.display());
    }
    let scenario_file = from_dir.join("scenario.json");
    if !scenario_file.is_file() {
        bail!(
            "scenario copy: no scenario.json at {} (corrupt directory?)",
            scenario_file.display()
        );
    }
    let to_dir = crate::paths::scenario_dir(to_sid)?;
    if to_dir.exists() {
        bail!(
            "scenario copy: destination already exists at {} (refusing to overwrite)",
            to_dir.display()
        );
    }
    fs::create_dir_all(&to_dir).with_context(|| format!("create {}", to_dir.display()))?;
    // Copy scenario.json with the id field patched.
    let body = fs::read_to_string(&scenario_file)
        .with_context(|| format!("read {}", scenario_file.display()))?;
    let mut parsed: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("parse {}", scenario_file.display()))?;
    if let Some(obj) = parsed.as_object_mut() {
        obj.insert("id".into(), serde_json::Value::String(to_sid.to_string()));
    }
    let patched = serde_json::to_string_pretty(&parsed)?;
    fs::write(to_dir.join("scenario.json"), format!("{patched}\n"))
        .with_context(|| format!("write {}", to_dir.join("scenario.json").display()))?;
    let copied = copy_scenario_assets(&from_dir, &to_dir)?;
    println!(
        "copied: {} → {}\nid: {:?} → {:?}\n(replays not copied; {} asset(s) copied)",
        scenario_file.display(),
        to_dir.join("scenario.json").display(),
        from_sid,
        to_sid,
        copied
    );
    Ok(0)
}

/// Baselines, packaged upload files, and the local inputs sidecar are
/// part of a scenario's meaning — a shot claim without its golden, an
/// upload literal without its file, or a {from: input} ref without its
/// local value all fail on the clone, so copies carry them. Replays
/// stay run history.
fn copy_scenario_assets(from_dir: &Path, to_dir: &Path) -> Result<usize> {
    let mut copied = 0usize;
    for dir in ["baselines", "files"] {
        let from = from_dir.join(dir);
        if from.is_dir() {
            let to = to_dir.join(dir);
            fs::create_dir_all(&to).with_context(|| format!("create {}", to.display()))?;
            for entry in fs::read_dir(&from)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                fs::copy(entry.path(), to.join(entry.file_name()))
                    .with_context(|| format!("copy {}", entry.path().display()))?;
                copied += 1;
            }
        }
    }
    // inputs.local.json is machine-local (gitignored) — carrying it
    // keeps {from: input} refs resolving on the clone.
    let inputs = from_dir.join("inputs.local.json");
    if inputs.is_file() {
        fs::copy(&inputs, to_dir.join("inputs.local.json"))
            .with_context(|| format!("copy {}", inputs.display()))?;
        copied += 1;
    }
    Ok(copied)
}

/// `scenario extract <sid>` — clone a scenario truncated at a run's failing
/// step, so a replay failure becomes a minimal standalone repro. `--through
/// <stepId>` cuts at an arbitrary step instead (e.g. extract a passing prefix
/// to seed `record continue`).
fn extract(
    sid: &str,
    run: Option<&str>,
    through: Option<&str>,
    to: Option<&str>,
    json: bool,
) -> Result<u8> {
    let from_dir = crate::paths::scenario_dir(sid)?;
    let scenario_file = from_dir.join("scenario.json");
    if !scenario_file.is_file() {
        bail!(
            "scenario extract: no scenario.json at {} (corrupt directory?)",
            scenario_file.display()
        );
    }
    let replays = from_dir.join("replays");
    let run_id = match run {
        Some(r) if r != "latest" => r.to_string(),
        _ => fs::read_to_string(replays.join("latest.txt"))
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("scenario extract: no runs for {sid:?} — replay it first"))?,
    };
    let events_path = replays.join(&run_id).join("events.jsonl");
    if !events_path.is_file() {
        bail!(
            "scenario extract: no events.jsonl for run {run_id:?} at {}",
            events_path.display()
        );
    }
    // Last write wins per idx — a step logs `running` then its outcome.
    let mut outcome: std::collections::BTreeMap<usize, (String, String)> =
        std::collections::BTreeMap::new();
    for line in fs::read_to_string(&events_path)
        .with_context(|| format!("read {}", events_path.display()))?
        .lines()
    {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(idx), Some(id), Some(status)) =
            (ev["idx"].as_u64(), ev["id"].as_str(), ev["status"].as_str())
        else {
            continue;
        };
        if status == "running" && outcome.contains_key(&(idx as usize)) {
            continue;
        }
        outcome.insert(idx as usize, (id.to_string(), status.to_string()));
    }
    if outcome.is_empty() {
        bail!("scenario extract: run {run_id:?} recorded no step events");
    }

    let body = fs::read_to_string(&scenario_file)
        .with_context(|| format!("read {}", scenario_file.display()))?;
    let mut parsed: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("parse {}", scenario_file.display()))?;
    let steps_len = parsed["steps"].as_array().map(|a| a.len()).unwrap_or(0);

    // events idx is the 1-based position in the steps array.
    let (cut_idx, cut_reason) = if let Some(step_id) = through {
        let pos = parsed["steps"]
            .as_array()
            .and_then(|a| a.iter().position(|s| s["id"].as_str() == Some(step_id)))
            .ok_or_else(|| anyhow!("no step with id {step_id:?} in {sid}"))?;
        (pos + 1, format!("through step {step_id:?}"))
    } else {
        let fail_idx = outcome
            .iter()
            .find(|(_, (_, status))| status == "fail")
            .map(|(idx, _)| *idx)
            .ok_or_else(|| {
                anyhow!(
                    "run {run_id:?} has no failing step — pass --through <stepId> to cut elsewhere"
                )
            })?;
        let step_id = outcome
            .get(&fail_idx)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();
        (fail_idx, format!("failing step {step_id:?}"))
    };
    if cut_idx > steps_len {
        bail!("run events exceed the scenario's {steps_len} step(s) — stale run?")
    }

    // Mint a free sid: <sid>-extract, then -2, -3, …
    let to_sid = match to {
        Some(t) => {
            if t.is_empty() || t.contains('/') || t.contains('\\') || t.starts_with('.') {
                bail!("scenario extract: --to {t:?} must be a non-empty, slash-free, non-dotfile name");
            }
            t.to_string()
        }
        None => {
            let mut candidate = format!("{sid}-extract");
            let mut n = 2;
            while crate::paths::scenario_dir(&candidate)
                .map(|p| p.exists())
                .unwrap_or(false)
            {
                candidate = format!("{sid}-extract-{n}");
                n += 1;
            }
            candidate
        }
    };
    let to_dir = crate::paths::scenario_dir(&to_sid)?;
    if to_dir.exists() {
        bail!(
            "scenario extract: destination already exists at {} (refusing to overwrite)",
            to_dir.display()
        );
    }
    fs::create_dir_all(&to_dir).with_context(|| format!("create {}", to_dir.display()))?;

    let total = steps_len;
    if let Some(arr) = parsed["steps"].as_array_mut() {
        arr.truncate(cut_idx);
    }
    let kept = parsed["steps"].as_array().map(|a| a.len()).unwrap_or(0);
    if let Some(obj) = parsed.as_object_mut() {
        obj.insert("id".into(), serde_json::Value::String(to_sid.clone()));
        let intent = obj["intent"].as_str().unwrap_or("").to_string();
        obj.insert(
            "intent".into(),
            serde_json::Value::String(format!("{intent} [extract: run {run_id}, {cut_reason}]")),
        );
        let tags = obj
            .entry("tags")
            .or_insert_with(|| serde_json::Value::Array(vec![]));
        if let Some(arr) = tags.as_array_mut() {
            arr.push(serde_json::Value::String("extract".into()));
        }
    }
    schema::validate_value(&parsed).context("extracted scenario failed schema validation")?;
    fs::write(
        to_dir.join("scenario.json"),
        format!("{}\n", serde_json::to_string_pretty(&parsed)?),
    )
    .with_context(|| format!("write {}", to_dir.join("scenario.json").display()))?;
    let copied_assets = copy_scenario_assets(&from_dir, &to_dir)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "from": sid,
                "to": to_sid,
                "run": run_id,
                "cutAt": cut_idx,
                "keptSteps": kept,
                "droppedSteps": total - kept,
                "assets": copied_assets,
            })
        );
    } else {
        println!(
            "extracted: {} → {}\nrun: {} ({})\nsteps: kept {}/{} ({} asset(s) copied)",
            scenario_file.display(),
            to_dir.join("scenario.json").display(),
            run_id,
            cut_reason,
            kept,
            total,
            copied_assets
        );
    }
    Ok(0)
}

fn rename(from_sid: &str, to_sid: &str) -> Result<u8> {
    if from_sid == to_sid {
        bail!("scenario rename: from and to are the same ({from_sid:?})");
    }
    if to_sid.is_empty() || to_sid.contains('/') || to_sid.contains('\\') || to_sid.starts_with('.')
    {
        bail!(
            "scenario rename: new sid {to_sid:?} must be a non-empty, slash-free, non-dotfile name"
        );
    }
    let from_dir = crate::paths::scenario_dir(from_sid)?;
    if !from_dir.is_dir() {
        bail!(
            "scenario rename: source not found at {}",
            from_dir.display()
        );
    }
    let to_dir = crate::paths::scenario_dir(to_sid)?;
    if to_dir.exists() {
        bail!(
            "scenario rename: destination already exists at {} (refusing to overwrite)",
            to_dir.display()
        );
    }
    // Renaming a bound scenario orphans the live buffer's sid — flush
    // would recreate the old dir as a zombie.
    if let Some(steps) = bound_recording_steps(from_sid)? {
        bail!(
            "scenario rename: {from_sid:?} is bound to the active recording \
             ({steps} step(s)) — `flush` it, or `start --force` to abandon it",
        );
    }
    let scenario_file = from_dir.join("scenario.json");
    if !scenario_file.is_file() {
        bail!(
            "scenario rename: no scenario.json at {} (corrupt directory?)",
            scenario_file.display()
        );
    }
    // 1) Patch the in-file id BEFORE moving the directory so a crash
    //    mid-rename still leaves a self-consistent directory.
    let body = fs::read_to_string(&scenario_file)
        .with_context(|| format!("read {}", scenario_file.display()))?;
    let mut parsed: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("parse {}", scenario_file.display()))?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| anyhow!("scenario.json root must be an object"))?;
    let old_id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_default();
    obj.insert("id".into(), serde_json::Value::String(to_sid.to_string()));
    let patched = serde_json::to_string_pretty(&parsed)?;
    fs::write(&scenario_file, format!("{patched}\n"))
        .with_context(|| format!("write {}", scenario_file.display()))?;

    // 2) Move the directory.
    fs::rename(&from_dir, &to_dir)
        .with_context(|| format!("rename {} → {}", from_dir.display(), to_dir.display()))?;

    println!(
        "renamed: {} → {}\nid: {:?} → {:?}",
        from_dir.display(),
        to_dir.display(),
        old_id,
        to_sid
    );
    Ok(0)
}

/// `scenario tag` — list or mutate a scenario's `tags[]` (the field
/// `replay --tags` selects on). With no --add/--remove it prints the
/// current set. An emptied set drops the field entirely.
fn tag(sid: &str, add: &[String], remove: &[String], json_out: bool) -> Result<u8> {
    let dir = crate::paths::scenario_dir(sid)?;
    if !dir.is_dir() {
        bail!("scenario tag: not found at {}", dir.display());
    }
    let scenario_file = dir.join("scenario.json");
    let body = fs::read_to_string(&scenario_file)
        .with_context(|| format!("read {}", scenario_file.display()))?;
    let mut parsed: serde_json::Value = serde_json::from_str(&body)
        .with_context(|| format!("parse {}", scenario_file.display()))?;
    let obj = parsed
        .as_object_mut()
        .ok_or_else(|| anyhow!("scenario.json root must be an object"))?;

    let mut tags: Vec<String> = obj
        .get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    if add.is_empty() && remove.is_empty() {
        if json_out {
            println!("{}", serde_json::to_string(&tags)?);
        } else if tags.is_empty() {
            println!("(no tags)");
        } else {
            for t in &tags {
                println!("{t}");
            }
        }
        return Ok(0);
    }

    if let Some(steps) = bound_recording_steps(sid)? {
        eprintln!(
            "warning: {sid:?} is bound to the active recording ({steps} step(s)) — `buffer load` again after editing, or the next flush will overwrite this change"
        );
    }

    for t in add {
        if !tags.contains(t) {
            tags.push(t.clone());
        }
    }
    tags.retain(|t| !remove.contains(t));
    tags.sort();
    tags.dedup();

    if tags.is_empty() {
        obj.remove("tags");
    } else {
        obj.insert(
            "tags".into(),
            serde_json::Value::Array(
                tags.iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    let patched = serde_json::to_string_pretty(&parsed)?;
    fs::write(&scenario_file, format!("{patched}\n"))
        .with_context(|| format!("write {}", scenario_file.display()))?;

    if json_out {
        println!("{}", serde_json::to_string(&tags)?);
    } else {
        println!(
            "tags: {}",
            if tags.is_empty() {
                "(none)".to_string()
            } else {
                tags.join(", ")
            }
        );
    }
    Ok(0)
}

/// Shorthand for the bound-recording check shared by the scenario
/// mutators (delete/rename refuse; in-place edits warn).
fn bound_recording_steps(sid: &str) -> Result<Option<usize>> {
    crate::recorder_state::RecorderState::bound_steps(sid)
}

fn delete(sid: &str, confirmed: bool) -> Result<u8> {
    if sid.is_empty() || sid.contains('/') || sid.contains('\\') || sid.starts_with('.') {
        bail!("scenario delete: sid {sid:?} must be non-empty, slash-free, non-dotfile");
    }
    let dir = crate::paths::scenario_dir(sid)?;
    if !dir.is_dir() {
        bail!("scenario delete: not found at {}", dir.display());
    }
    // Deleting the scenario an active recording loaded (or owns) orphans
    // the live buffer — and flush would then recreate the dir as a
    // zombie. Refuse while the state file points at this sid.
    if let Some(steps) = bound_recording_steps(sid)? {
        bail!(
            "scenario delete: {sid:?} is bound to the active recording \
             ({steps} step(s)) — `flush` it, or `start --force` to abandon it",
        );
    }
    let replays = crate::paths::run_dirs(&dir.join("replays")).len();
    if !confirmed {
        println!(
            "would delete: {}\n  ({} replay(s) under replays/)\nre-run with --yes / -y to confirm.",
            dir.display(),
            replays
        );
        return Ok(0);
    }
    fs::remove_dir_all(&dir).with_context(|| format!("remove_dir_all {}", dir.display()))?;
    println!(
        "deleted: {} ({} replay(s) discarded)",
        dir.display(),
        replays
    );
    Ok(0)
}

fn prune_all(
    keep: usize,
    confirmed: bool,
    keep_failed: bool,
    root_override: Option<&std::path::Path>,
) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let entries: Vec<std::path::PathBuf> = match std::fs::read_dir(&root) {
        Ok(it) => it
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    if entries.is_empty() {
        println!("prune-all: nothing under {} \u{2014} done", root.display());
        return Ok(0);
    }
    let mut total_dropped = 0usize;
    let mut visited = 0usize;
    for dir in entries {
        let Some(sid) = dir.file_name().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        visited += 1;
        // Reuse prune_replays' counting + drop logic by computing the
        // same set here; we don't recurse into prune_replays because
        // it would re-resolve <sid> via paths::scenario_dir (cheap, but
        // also prints its own per-sid header line which would be very
        // noisy across N scenarios).
        let replays_dir = dir.join("replays");
        let runs = crate::paths::run_dirs(&replays_dir);
        if runs.len() <= keep {
            continue;
        }
        let candidate_drop = runs.len() - keep;
        let mut victims: Vec<std::path::PathBuf> = runs[..candidate_drop].to_vec();
        if keep_failed {
            victims.retain(|v| {
                let bytes = match std::fs::read(v.join("audit.json")) {
                    Ok(b) => b,
                    Err(_) => return true,
                };
                match serde_json::from_slice::<serde_json::Value>(&bytes) {
                    Ok(val) => {
                        // Drop iff exitCode is present and 0 (passed) or
                        // absent (unknown). Retain iff failed.
                        let failed = val
                            .get("exitCode")
                            .and_then(|c| c.as_i64())
                            .is_some_and(|c| c != 0);
                        !failed
                    }
                    Err(_) => true,
                }
            });
        }
        let drop_count = victims.len();
        if drop_count == 0 {
            continue;
        }
        if !confirmed {
            println!(
                "would prune {drop_count} replay(s) under {} (sid={sid}, keep {keep})",
                replays_dir.display()
            );
            for v in &victims {
                println!("  - {}", v.display());
            }
        } else {
            for v in &victims {
                fs::remove_dir_all(v).with_context(|| format!("remove_dir_all {}", v.display()))?;
            }
            println!(
                "pruned {drop_count} replay(s) under {} (sid={sid}, {keep} kept)",
                replays_dir.display()
            );
        }
        total_dropped += drop_count;
    }
    println!(
        "\nprune-all: visited {visited} scenario(s); {} {total_dropped} replay(s) (keep={keep})",
        if confirmed { "dropped" } else { "would drop" }
    );
    if !confirmed && total_dropped > 0 {
        println!("re-run with --yes / -y to confirm.");
    }
    Ok(0)
}

fn prune_replays(sid: &str, keep: usize, confirmed: bool, keep_failed: bool) -> Result<u8> {
    let dir = crate::paths::scenario_dir(sid)?;
    if !dir.is_dir() {
        bail!("scenario prune-replays: not found at {}", dir.display());
    }
    let replays_dir = dir.join("replays");
    let entries = crate::paths::run_dirs(&replays_dir);
    let is_failed = |run_dir: &std::path::Path| -> bool {
        let audit_path = run_dir.join("audit.json");
        let bytes = match std::fs::read(&audit_path) {
            Ok(b) => b,
            Err(_) => return false,
        };
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => v
                .get("exitCode")
                .and_then(|c| c.as_i64())
                .is_some_and(|c| c != 0),
            Err(_) => false,
        }
    };
    if entries.len() <= keep {
        println!(
            "prune-replays: {} (have {}, keep {}) — nothing to do",
            replays_dir.display(),
            entries.len(),
            keep
        );
        return Ok(0);
    }
    let drop_count = entries.len() - keep;
    let mut victims: Vec<std::path::PathBuf> = entries[..drop_count].to_vec();
    if keep_failed {
        victims.retain(|v| !is_failed(v));
    }
    let drop_count = victims.len();
    if drop_count == 0 {
        println!(
            "prune-replays: {} (nothing to drop after --keep-failed filter)",
            replays_dir.display()
        );
        return Ok(0);
    }
    if !confirmed {
        println!(
            "would prune {} replay(s) under {} (keep {} most recent):",
            drop_count,
            replays_dir.display(),
            keep
        );
        for v in &victims {
            println!("  - {}", v.display());
        }
        println!("re-run with --yes / -y to confirm.");
        return Ok(0);
    }
    let mut removed = 0usize;
    for v in &victims {
        fs::remove_dir_all(v).with_context(|| format!("remove_dir_all {}", v.display()))?;
        removed += 1;
    }
    println!(
        "pruned {} replay(s) under {} ({} kept)",
        removed,
        replays_dir.display(),
        keep
    );
    Ok(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LintFormat {
    Text,
    Json,
    Github,
}

pub(crate) fn parse_lint_format(s: &str) -> Result<LintFormat> {
    match s {
        "text" => Ok(LintFormat::Text),
        "json" => Ok(LintFormat::Json),
        "github" => Ok(LintFormat::Github),
        other => bail!("--format expects 'text', 'json', or 'github', got {other:?}"),
    }
}

fn list_lint_rules(json_out: bool) -> Result<u8> {
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Rule {
        code: &'static str,
        severity: &'static str,
        description: &'static str,
    }
    let rules = [
        Rule {
            code: "duplicate-step-id",
            severity: "error",
            description: "Two or more steps share the same id.",
        },
        Rule {
            code: "empty-intent",
            severity: "warning",
            description: "A step has a whitespace-only intent string.",
        },
        Rule {
            code: "bare-do",
            severity: "warning",
            description: "A do step is not followed by a check (trailing or pre-do).",
        },
        Rule {
            code: "no-visual-check",
            severity: "warning",
            description: "A scenario with do steps has no { shot: ... } claim — nothing pixel-diffs a baseline.",
        },
        Rule {
            code: "shot-missing-baseline",
            severity: "error",
            description: "A { shot: <stepId> } claim has no <scenario>/baselines/<stepId>.png — replay always fails the claim.",
        },
        Rule {
            code: "undeclared-input",
            severity: "error",
            description: "A value references inputs.<name> not declared on the scenario.",
        },
        Rule {
            code: "unused-input",
            severity: "warning",
            description: "A declared input is never referenced.",
        },
        Rule {
            code: "undeclared-step-ref",
            severity: "error",
            description: "A value references { from: 'step', stepId: <id> } where the id doesn't exist.",
        },
        Rule {
            code: "goto-without-url",
            severity: "error",
            description: "A do/goto step has no value.",
        },
        Rule {
            code: "missing-locator",
            severity: "error",
            description: "A do step that needs a locator (click, type, clear, hover, focus, blur, check, uncheck) has no on locator.",
        },
        Rule {
            code: "params-on-noop",
            severity: "warning",
            description: "A do step whose verb ignores params (reload, back, forward) has a non-empty params object.",
        },
        Rule {
            code: "no-env-open",
            severity: "warning",
            description: "Scenario has no env.open[] entries; replay will start with no landing page.",
        },
        Rule {
            code: "no-navigation",
            severity: "warning",
            description: "Scenario never navigates: no env.open nav op and no do/goto step, so replay runs against whatever page the session is already on.",
        },
        Rule {
            code: "no-checks",
            severity: "warning",
            description: "Scenario has no check steps; replay can only fail on browser errors, not assertions.",
        },
        Rule {
            code: "empty-steps",
            severity: "warning",
            description: "Scenario has zero steps; replay will only open env then close it.",
        },
        Rule {
            code: "wait-without-condition",
            severity: "warning",
            description: "A do/wait step has none of params.ms/until/url/idle/idleMs/timeoutMs/locator; falls back to a networkidle wait that may miss the intended condition.",
        },
        Rule {
            code: "shot-without-baseline",
            severity: "warning",
            description: "A {shot: <stepId>} claim has no baselines/<stepId>.png beside scenario.json; replay will fail with a missing-baseline hint. Skipped for stdin input.",
        },
        Rule {
            code: "domshot-without-baseline",
            severity: "warning",
            description: "A {domshot: <stepId>} claim has no baselines/<stepId>.snap.txt beside scenario.json; replay will fail with a missing-baseline hint. Skipped for stdin input.",
        },
        Rule {
            code: "orphan-baseline",
            severity: "warning",
            description: "baselines/<stepId>.png or .snap.txt exists but no shot/domshot claim references <stepId> — a stale golden left by a deleted or renamed step. Skipped for stdin input.",
        },
        Rule {
            code: "upload-file-missing",
            severity: "warning",
            description: "An upload/fileChooser step references a path that doesn't exist — replay fails at the step. Skipped for stdin input and for {{...}} or ${...} templated paths.",
        },
        Rule {
            code: "upload-file-absolute",
            severity: "warning",
            description: "An upload/fileChooser step references an absolute path — it replays on this machine only; package it under the scenario's files/ dir and reference files/<name>.",
        },
        Rule {
            code: "brittle-locator",
            severity: "warning",
            description: "A raw css/xpath locator is positional or generated-looking (xpath [N]/last()/position(), css :nth-* chains, #id with a digit/hash tail). Self-heal can't rescue these — prefer role+name, text, or a stable css/testid.",
        },
        Rule {
            code: "hardcoded-secret",
            severity: "warning",
            description: "A do/type step targets a password-shaped field (any locator string containing 'password') while carrying a plain {from: literal} value — the secret sits verbatim in scenario.json. Run 'scenario redact' to move it into a sensitive input.",
        },
        Rule {
            code: "fixed-sleep",
            severity: "warning",
            description: "A do/wait step whose only condition is params.ms — a fixed delay flakes when the app is slow and idles when it is fast. Gate on the outcome instead: params.until/url/idle/locator.",
        },
    ];
    if json_out {
        println!("{}", serde_json::to_string_pretty(&rules)?);
        return Ok(0);
    }
    println!("agent-qa scenario lint rules ({})", rules.len());
    for r in &rules {
        let code = r.code;
        let sev = r.severity;
        let desc = r.description;
        println!("  [{sev}] {code}: {desc}");
    }
    Ok(0)
}

/// Does the scenario reach a page on its own? `start --open <url>` drives the
/// recording browser without recording anything, so a scenario can flush with
/// every check intact and no way to reach the page those checks describe.
fn scenario_navigates(j: &Scenario) -> bool {
    use crate::scenario::{EnvOp, Step, Verb};
    let env_navigates = j
        .env
        .as_ref()
        .and_then(|e| e.open.as_ref())
        .is_some_and(|ops| ops.iter().any(|o| matches!(o, EnvOp::Nav { .. })));
    env_navigates
        || j.steps.iter().any(|s| {
            matches!(
                s,
                Step::Do {
                    verb: Verb::Goto,
                    ..
                }
            )
        })
}

/// Collect every `{ "raw": { "kind": "css"|"xpath", "value": <str> } }` shape
/// in a JSON subtree — that pair is the locator envelope everywhere it can
/// appear (do `on`, claim `element`, shot `clip`, `params.to`, role scopes).
fn walk_raw_locators(v: &serde_json::Value, out: &mut Vec<(String, String)>) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(raw) = m.get("raw").and_then(|r| r.as_object()) {
                let kind = raw.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                let value = raw.get("value").and_then(|s| s.as_str()).unwrap_or("");
                if !value.is_empty() && (kind == "css" || kind == "xpath") {
                    out.push((kind.to_string(), value.to_string()));
                }
            }
            for child in m.values() {
                walk_raw_locators(child, out);
            }
        }
        serde_json::Value::Array(a) => {
            for child in a {
                walk_raw_locators(child, out);
            }
        }
        _ => {}
    }
}

/// A locator is brittle when a page change removes every stable signal:
/// positional xpath predicates break on a sibling reorder, css `:nth-*`
/// chains on a wrapper insert, generated-looking ids on the next deploy.
fn brittle_locator_reason(kind: &str, value: &str) -> Option<&'static str> {
    if kind == "xpath" {
        let mut in_bracket = false;
        let mut buf = String::new();
        for c in value.chars() {
            if in_bracket {
                if c == ']' {
                    in_bracket = false;
                    let t = buf.trim();
                    if (!t.is_empty() && t.chars().all(|ch| ch.is_ascii_digit()))
                        || t.starts_with("last()")
                        || t.starts_with("position()")
                    {
                        return Some("xpath uses a positional predicate ([N]/last()/position())");
                    }
                    buf.clear();
                } else {
                    buf.push(c);
                }
            } else if c == '[' {
                in_bracket = true;
            }
        }
        return None;
    }
    // css: a chain of 2+ positional pseudos, or an id that looks generated
    // (ends in 3+ digits or is a hex/uuid run) — e.g. #ember982, #a1b2c3d4.
    if value.matches(":nth-").count() >= 2 {
        return Some("css chains multiple positional selectors (:nth-*)");
    }
    let mut i = 0;
    let bytes = value.as_bytes();
    let mut depth = 0usize;
    let mut quote = 0u8;
    while i < bytes.len() {
        let c = bytes[i];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' | b'\'' => quote = c,
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b'#' if depth == 0 => {
                let id: String = value[i + 1..]
                    .chars()
                    .take_while(|ch| ch.is_alphanumeric() || *ch == '-' || *ch == '_' || *ch == ':')
                    .collect();
                let hexish = id.len() >= 8
                    && id
                        .chars()
                        .all(|ch| ch.is_ascii_hexdigit() || ch == '-' || ch == '_');
                let digit_tail = id
                    .chars()
                    .rev()
                    .take_while(|ch| ch.is_ascii_digit())
                    .count()
                    >= 3;
                if hexish || digit_tail {
                    return Some(
                        "id selector looks generated (#…hex/digits) — it drifts between deploys",
                    );
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Finding {
    severity: &'static str,
    code: &'static str,
    message: String,
}

/// Runs every lint rule against one scenario file and returns the raw
/// findings, unfiltered by --rule/--exclude-rule. Shared by `lint`
/// (renders) and `lint_collect` (counts for `scenario check`) so a new
/// rule cannot drift between the two paths.
fn lint_findings(path: &Path) -> Result<(Vec<Finding>, Scenario)> {
    // For stdin ('-'), buffer once via io::stdin_or_path; the guard
    // keeps the tempfile alive for the rest of this function.
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    // Walk the raw JSON tree once so we can collect input references
    // without depending on the typed Scenario shape (which would force us
    // to teach the lint about every Value variant).
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let j = load_scenario(path)?;

    let mut findings: Vec<Finding> = Vec::new();
    use crate::scenario::Step;
    use std::collections::{HashMap, HashSet};

    // 1) duplicate step ids
    let mut id_counts: HashMap<&str, usize> = HashMap::new();
    for step in &j.steps {
        *id_counts.entry(step.id()).or_insert(0) += 1;
    }
    for (id, n) in &id_counts {
        if *n > 1 {
            findings.push(Finding {
                severity: "error",
                code: "duplicate-step-id",
                message: format!("step id {id:?} appears {n} times"),
            });
        }
    }

    // 2) empty step intent
    for step in &j.steps {
        if step.intent().trim().is_empty() {
            findings.push(Finding {
                severity: "warning",
                code: "empty-intent",
                message: format!("step {:?} has an empty intent", step.id()),
            });
        }
    }

    // 3) bare do (trailing or pre-do) — same heuristic coverage uses.
    let mut prev_was_do_id: Option<&str> = None;
    for step in &j.steps {
        match step {
            Step::Do { .. } => {
                if let Some(id) = prev_was_do_id {
                    findings.push(Finding {
                        severity: "warning",
                        code: "bare-do",
                        message: format!("step {id:?} is a do not followed by a check"),
                    });
                }
                prev_was_do_id = Some(step.id());
            }
            Step::Check { .. } => {
                prev_was_do_id = None;
            }
        }
    }
    if let Some(id) = prev_was_do_id {
        findings.push(Finding {
            severity: "warning",
            code: "bare-do",
            message: format!("step {id:?} is a trailing do not followed by a check"),
        });
    }

    // 3b) no golden coverage — a scenario with do steps but zero shot/domshot
    // claims has no baseline at all; nudge toward `flush --auto-shots` /
    // editor camera (or a domshot claim for a structural golden).
    let has_do = j.steps.iter().any(|s| matches!(s, Step::Do { .. }));
    let has_golden = j.steps.iter().any(|s| {
        matches!(
            s,
            Step::Check {
                claim: crate::scenario::Claim {
                    subject: crate::scenario::ClaimSubject::Shot { .. }
                        | crate::scenario::ClaimSubject::Domshot { .. },
                    ..
                },
                ..
            }
        )
    });
    if has_do && !has_golden {
        findings.push(Finding {
            severity: "warning",
            code: "no-visual-check",
            message: "scenario has do steps but no { shot: ... } / { domshot: ... } claim — add one for golden coverage (`flush --auto-shots` covers every do step)".to_string(),
        });
    }

    // 3c) shot claim with no minted baseline — guaranteed replay failure.
    // Only meaningful when the lint target is a real `<sid>/scenario.json`
    // on disk (stdin has no scenario dir, so baselines can't be resolved).
    if path.file_name().is_some_and(|n| n == "scenario.json") {
        let baselines = path
            .parent()
            .map(|d| d.join("baselines"))
            .unwrap_or_default();
        for step in &j.steps {
            if let Step::Check { id, claim, .. } = step {
                if let crate::scenario::ClaimSubject::Shot { shot, .. } = &claim.subject {
                    if !baselines.join(format!("{shot}.png")).is_file() {
                        findings.push(Finding {
                            severity: "error",
                            code: "shot-missing-baseline",
                            message: format!(
                                "step {id:?} claims shot {shot:?} but baselines/{shot}.png is missing — mint it with `shot-accept <sid>`"
                            ),
                        });
                    }
                }
            }
        }
    }

    // 3d) brittle locators — walk every locator-shaped object in the step
    // list (on, claim element, shot clip, params.to, role scopes — all carry
    // `raw: {kind, value}`). These are the locators self-heal cannot rescue:
    // a sibling reorder or a regenerated id leaves no stable signal.
    if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let sid = step.get("id").and_then(|i| i.as_str()).unwrap_or("?");
            let mut locs: Vec<(String, String)> = Vec::new();
            walk_raw_locators(step, &mut locs);
            for (kind, value) in locs {
                if let Some(reason) = brittle_locator_reason(&kind, &value) {
                    findings.push(Finding {
                        severity: "warning",
                        code: "brittle-locator",
                        message: format!(
                            "step {sid:?} locator {value:?}: {reason} — prefer role+name, text, or a stable css/testid"
                        ),
                    });
                }
            }
        }
    }

    // 3e) fixed sleeps — a do/wait whose only condition is `ms` guesses at
    // timing; it flakes when the app is slow and idles when it is fast.
    // Wait on the outcome instead (until/url/idle/locator).
    if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let is_wait = step
                .get("verb")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v == "wait");
            if !is_wait {
                continue;
            }
            let params = step.get("params");
            let get = |k: &str| params.and_then(|p| p.get(k));
            if get("ms").is_some()
                && get("until").is_none()
                && get("url").is_none()
                && get("idle").is_none()
                && get("idleMs").is_none()
                && get("locator").is_none()
            {
                let sid = step.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                findings.push(Finding {
                    severity: "warning",
                    code: "fixed-sleep",
                    message: format!(
                        "step {sid:?} waits {ms}ms with no condition — prefer params.until/url/idle/locator so the step gates on the outcome, not the clock",
                        ms = get("ms").and_then(|m| m.as_u64()).unwrap_or(0)
                    ),
                });
            }
        }
    }

    // 3f) hardcoded secrets — a type step on a password-shaped field whose
    // value is a plain {from: literal} puts the secret verbatim in a file
    // that gets committed. `scenario redact` rewrites it as a sensitive
    // inputs ref.
    if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let is_type = step
                .get("verb")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v == "type");
            if !is_type {
                continue;
            }
            let literal = step
                .get("value")
                .and_then(|v| v.get("literal"))
                .and_then(|l| l.as_str());
            if !literal.is_some_and(|l| !l.is_empty()) {
                continue;
            }
            let Some(on) = step.get("on") else { continue };
            // Any locator string mentioning "password" counts — css
            // `input[type=password]`, a role name like "Password", or a
            // testid. Conservative: no hint, no finding.
            let mut hints_password = false;
            let mut stack = vec![on];
            while let Some(v) = stack.pop() {
                match v {
                    serde_json::Value::String(s) if s.to_lowercase().contains("password") => {
                        hints_password = true;
                        break;
                    }
                    serde_json::Value::Object(m) => stack.extend(m.values()),
                    serde_json::Value::Array(a) => stack.extend(a.iter()),
                    _ => {}
                }
            }
            if hints_password {
                let sid = step.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                findings.push(Finding {
                    severity: "warning",
                    code: "hardcoded-secret",
                    message: format!(
                        "step {sid:?} types a literal into a password-shaped field — the secret sits verbatim in scenario.json; run 'agent-qa scenario redact <file> --name <VAR> --step {sid}' to move it into a sensitive input"
                    ),
                });
            }
        }
    }

    // 3f) hardcoded secrets — a type step on a password-shaped field whose
    // value is a plain {from: literal} puts the secret verbatim in a file
    // that gets committed. `scenario redact` rewrites it as a sensitive
    // inputs ref.
    if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            let is_type = step
                .get("verb")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v == "type");
            if !is_type {
                continue;
            }
            let literal = step
                .get("value")
                .and_then(|v| v.get("literal"))
                .and_then(|l| l.as_str());
            if !literal.is_some_and(|l| !l.is_empty()) {
                continue;
            }
            let Some(on) = step.get("on") else { continue };
            // Any locator string mentioning "password" counts — css
            // `input[type=password]`, a role name like "Password", or a
            // testid. Conservative: no hint, no finding.
            let mut hints_password = false;
            let mut stack = vec![on];
            while let Some(v) = stack.pop() {
                match v {
                    serde_json::Value::String(s) if s.to_lowercase().contains("password") => {
                        hints_password = true;
                        break;
                    }
                    serde_json::Value::Object(m) => stack.extend(m.values()),
                    serde_json::Value::Array(a) => stack.extend(a.iter()),
                    _ => {}
                }
            }
            if hints_password {
                let sid = step.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                findings.push(Finding {
                    severity: "warning",
                    code: "hardcoded-secret",
                    message: format!(
                        "step {sid:?} types a literal into a password-shaped field — the secret sits verbatim in scenario.json; run 'agent-qa scenario redact <file> --name <VAR> --step {sid}' to move it into a sensitive input"
                    ),
                });
            }
        }
    }

    // 3f) claim value carrying a do-step `{"from": ...}` spec — claim values
    // are plain JSON; an object spec serializes verbatim and never matches,
    // so the check fails with a confusing "expected to contain {from:…}".
    if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
        for step in steps {
            if step.get("kind").and_then(|k| k.as_str()) != Some("check") {
                continue;
            }
            let looks_like_spec = step
                .get("claim")
                .and_then(|c| c.get("value"))
                .and_then(|v| v.as_object())
                .is_some_and(|o| o.contains_key("from"));
            if looks_like_spec {
                let sid = step.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                findings.push(Finding {
                    severity: "error",
                    code: "claim-value-spec",
                    message: format!(
                        "step {sid:?} claim value is a do-step {{\"from\":…}} spec — claims take plain JSON (e.g. \"value\": \"example.com\")"
                    ),
                });
            }
        }
    }

    // 4) input references vs declarations
    let declared: HashSet<String> = j
        .inputs
        .as_ref()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let mut referenced: HashSet<String> = HashSet::new();
    collect_input_refs(&raw, &mut referenced);
    for input in &referenced {
        if !declared.contains(input) {
            findings.push(Finding {
                severity: "error",
                code: "undeclared-input",
                message: format!("input {input:?} is referenced but not declared"),
            });
        }
    }
    for input in &declared {
        if !referenced.contains(input) {
            findings.push(Finding {
                severity: "warning",
                code: "unused-input",
                message: format!("input {input:?} is declared but never referenced"),
            });
        }
    }

    // 5) dangling { from: 'step', stepId: '<id>' } references
    let step_ids: HashSet<String> = j.steps.iter().map(|s| s.id().to_string()).collect();
    let mut step_refs: HashSet<String> = HashSet::new();
    collect_step_refs(&raw, &mut step_refs);
    for r in &step_refs {
        if !step_ids.contains(r) {
            findings.push(Finding {
                severity: "error",
                code: "undeclared-step-ref",
                message: format!(
                    "value references stepId {r:?} which doesn't exist in this scenario"
                ),
            });
        }
    }

    // 6) goto step with no value
    for step in &j.steps {
        if let Step::Do {
            id,
            verb: crate::scenario::Verb::Goto,
            value,
            ..
        } = step
        {
            if value.is_none() {
                findings.push(Finding {
                    severity: "error",
                    code: "goto-without-url",
                    message: format!("step {id:?} is verb=goto but has no value"),
                });
            }
        }
    }

    // 7) locator-requiring verb with no on locator
    for step in &j.steps {
        if let Step::Do { id, verb, on, .. } = step {
            let needs_locator = matches!(
                verb,
                crate::scenario::Verb::Click
                    | crate::scenario::Verb::Type
                    | crate::scenario::Verb::Clear
                    | crate::scenario::Verb::Hover
                    | crate::scenario::Verb::Focus
                    | crate::scenario::Verb::Blur
                    | crate::scenario::Verb::Check
                    | crate::scenario::Verb::Uncheck
            );
            if !needs_locator {
                continue;
            }
            if on.is_none() {
                let verb_label = format!("{verb:?}").to_ascii_lowercase();
                findings.push(Finding {
                    severity: "error",
                    code: "missing-locator",
                    message: format!("step {id:?} verb={verb_label} requires an on locator"),
                });
            }
        }
    }

    // 8) params on a verb that ignores them (reload/back/forward)
    for step in &j.steps {
        if let Step::Do {
            id, verb, params, ..
        } = step
        {
            let is_noop = matches!(
                verb,
                crate::scenario::Verb::Reload
                    | crate::scenario::Verb::Back
                    | crate::scenario::Verb::Forward
            );
            if !is_noop {
                continue;
            }
            let has_params = params.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
            if has_params {
                let verb_label = format!("{verb:?}").to_ascii_lowercase();
                findings.push(Finding {
                    severity: "warning",
                    code: "params-on-noop",
                    message: format!(
                        "step {id:?} verb={verb_label} ignores params; remove for clarity"
                    ),
                });
            }
        }
    }

    // 9) scenario has no env.open[] entries
    let open_count = j
        .env
        .as_ref()
        .and_then(|e| e.open.as_ref())
        .map(|v| v.len())
        .unwrap_or(0);
    if open_count == 0 {
        findings.push(Finding {
            severity: "warning",
            code: "no-env-open",
            message: "scenario has no env.open[] entries; replay will start with no landing page"
                .into(),
        });
    }

    // 10) scenario has no check steps at all
    let check_count = j
        .steps
        .iter()
        .filter(|s| matches!(s, Step::Check { .. }))
        .count();
    if !j.steps.is_empty() && check_count == 0 {
        findings.push(Finding {
            severity: "warning",
            code: "no-checks",
            message:
                "scenario has no check steps; replay can only fail on browser errors, not assertions"
                    .into(),
        });
    }

    // 10b) scenario never navigates. `start --open <url>` drives the browser
    // without recording anything, so a scenario recorded that way flushes with
    // a landing page that exists only in the recording session — replay lands
    // wherever the session happens to be, and the checks pass or fail against
    // the wrong page.
    if !j.steps.is_empty() && !scenario_navigates(&j) {
        findings.push(Finding {
            severity: "warning",
            code: "no-navigation",
            message:
                "scenario never navigates (no env.open nav op, no do/goto step); replay runs against whatever page the session is already on"
                    .into(),
        });
    }

    // 11) scenario has zero steps
    if j.steps.is_empty() {
        findings.push(Finding {
            severity: "warning",
            code: "empty-steps",
            message: "scenario has zero steps; replay will only open env then close it".into(),
        });
    }

    // 12) wait step with neither params.timeoutMs nor params.locator
    for step in &j.steps {
        if let Step::Do {
            id,
            verb: crate::scenario::Verb::Wait,
            params,
            ..
        } = step
        {
            let has_condition = params
                .as_ref()
                .map(|p| {
                    p.get("ms").is_some()
                        || p.get("until").is_some()
                        || p.get("url").is_some()
                        || p.get("timeoutMs").is_some()
                        || p.get("locator").is_some()
                        || p.get("idle").is_some()
                        || p.get("idleMs").is_some()
                })
                .unwrap_or(false);
            if !has_condition {
                findings.push(Finding {
                    severity: "warning",
                    code: "wait-without-condition",
                    message: format!(
                        "step {id:?} verb=wait has no wait condition (params.ms/until/url/idle/idleMs/timeoutMs/locator); falls back to networkidle"
                    ),
                });
            }
        }
    }

    // 13) a {"shot": <stepId>} / {"domshot": <stepId>} claim needs a
    // committed baseline — replay fails on the missing file anyway; flag
    // it while the author still has the terminal in hand. Skipped on
    // stdin ('-'): the tempfile has no scenario dir to resolve baselines/
    // against. Nested claims (inside group/loop params.steps) count too —
    // same JSON walk the runner uses to find them.
    let from_stdin = matches!(&_guard, crate::io::StdinOrPath::Stdin { .. });
    if !from_stdin {
        if let Some(scenario_dir) = path.parent() {
            let mut shot_ids: Vec<String> = Vec::new();
            let mut domshot_ids: Vec<String> = Vec::new();
            fn collect_shots(
                v: &serde_json::Value,
                shots: &mut Vec<String>,
                domshots: &mut Vec<String>,
            ) {
                match v {
                    serde_json::Value::Object(map) => {
                        if let Some(subject) = map.get("claim").and_then(|c| c.get("subject")) {
                            if let Some(sid) = subject.get("shot").and_then(|s| s.as_str()) {
                                shots.push(sid.to_string());
                            }
                            if let Some(sid) = subject.get("domshot").and_then(|s| s.as_str()) {
                                domshots.push(sid.to_string());
                            }
                        }
                        for v in map.values() {
                            collect_shots(v, shots, domshots);
                        }
                    }
                    serde_json::Value::Array(arr) => {
                        for v in arr {
                            collect_shots(v, shots, domshots);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(steps) = raw.get("steps") {
                collect_shots(steps, &mut shot_ids, &mut domshot_ids);
            }
            let baselines = scenario_dir.join("baselines");
            for sid in &shot_ids {
                if !baselines.join(format!("{sid}.png")).is_file() {
                    findings.push(Finding {
                        severity: "warning",
                        code: "shot-without-baseline",
                        message: format!(
                            "check claims shot {sid:?} but baselines/{sid}.png is missing — run `agent-qa shot-accept` after a replay"
                        ),
                    });
                }
            }
            for sid in &domshot_ids {
                if !baselines.join(format!("{sid}.snap.txt")).is_file() {
                    findings.push(Finding {
                        severity: "warning",
                        code: "domshot-without-baseline",
                        message: format!(
                            "check claims domshot {sid:?} but baselines/{sid}.snap.txt is missing — run `agent-qa domshot-accept` after a replay"
                        ),
                    });
                }
            }
            // Inverse: a baseline PNG whose stepId no shot claim references
            // is a stale golden — deleted steps and renumbered ids leave
            // them behind, and they cost review attention forever.
            if let Ok(rd) = fs::read_dir(&baselines) {
                let claimed: std::collections::BTreeSet<&str> =
                    shot_ids.iter().map(String::as_str).collect();
                for ent in rd.flatten() {
                    let name = ent.file_name();
                    let name = name.to_string_lossy();
                    let Some(stem) = name.strip_suffix(".png") else {
                        continue;
                    };
                    if !claimed.contains(stem) {
                        findings.push(Finding {
                            severity: "warning",
                            code: "orphan-baseline",
                            message: format!(
                                "baselines/{name} — no shot claim references step {stem:?}; delete it or the claim was renamed"
                            ),
                        });
                    }
                }
            }
            // Same inverse check for domshot text baselines (.snap.txt).
            if let Ok(rd) = fs::read_dir(&baselines) {
                let claimed: std::collections::BTreeSet<&str> =
                    domshot_ids.iter().map(String::as_str).collect();
                for ent in rd.flatten() {
                    let name = ent.file_name();
                    let name = name.to_string_lossy();
                    let Some(stem) = name.strip_suffix(".snap.txt") else {
                        continue;
                    };
                    if !claimed.contains(stem) {
                        findings.push(Finding {
                            severity: "warning",
                            code: "orphan-baseline",
                            message: format!(
                                "baselines/{name} — no domshot claim references step {stem:?}; delete it or the claim was renamed"
                            ),
                        });
                    }
                }
            }

            // 14) upload/fileChooser file refs — a path that doesn't
            // resolve at lint time fails at replay anyway; an absolute
            // path works only on the machine it was recorded on
            // (flush's files/ packaging exists for portability).
            let mut file_refs: Vec<(String, String)> = Vec::new(); // (stepId, path)
            if let Some(steps) = raw.get("steps").and_then(|s| s.as_array()) {
                for step in steps {
                    if step.get("kind").and_then(|k| k.as_str()) != Some("do") {
                        continue;
                    }
                    let step_id = step
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let node = match step.get("verb").and_then(|v| v.as_str()) {
                        Some("upload") => step.get("value").and_then(|v| v.get("literal")),
                        Some("fileChooser") => step.get("params").and_then(|p| p.get("files")),
                        _ => None,
                    };
                    let mut push = |s: &str| file_refs.push((step_id.clone(), s.to_string()));
                    match node {
                        Some(serde_json::Value::String(s)) => push(s),
                        Some(serde_json::Value::Array(items)) => {
                            for item in items.iter().filter_map(|i| i.as_str()) {
                                push(item);
                            }
                        }
                        _ => {}
                    }
                }
            }
            for (step_id, p) in file_refs {
                if p.contains("{{") || p.contains("${") {
                    continue; // templated — resolves at replay
                }
                let abs = Path::new(&p).is_absolute();
                let resolved = if abs {
                    Path::new(&p).to_path_buf()
                } else {
                    scenario_dir.join(&p)
                };
                if !resolved.is_file() {
                    findings.push(Finding {
                        severity: "warning",
                        code: "upload-file-missing",
                        message: format!(
                            "step {step_id:?} references file {p:?} — not found at {}; replay fails at this step",
                            resolved.display()
                        ),
                    });
                } else if abs {
                    findings.push(Finding {
                        severity: "warning",
                        code: "upload-file-absolute",
                        message: format!(
                            "step {step_id:?} references absolute path {p:?} — replays on this machine only; package under files/ and reference files/<name>"
                        ),
                    });
                }
            }
        }
    }

    Ok((findings, j))
}

pub(crate) fn lint(
    path: &Path,
    format: LintFormat,
    strict: bool,
    only_rules: Option<&[String]>,
    exclude_rules: Option<&[String]>,
) -> Result<u8> {
    let (mut findings, j) = lint_findings(path)?;

    if let Some(only) = only_rules {
        let set: std::collections::HashSet<&str> = only.iter().map(String::as_str).collect();
        findings.retain(|f| set.contains(f.code));
    }
    if let Some(excl) = exclude_rules {
        let set: std::collections::HashSet<&str> = excl.iter().map(String::as_str).collect();
        findings.retain(|f| !set.contains(f.code));
    }
    let errors = findings.iter().filter(|f| f.severity == "error").count();
    let warnings = findings.iter().filter(|f| f.severity == "warning").count();

    match format {
        LintFormat::Json => {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Report<'a> {
                id: &'a str,
                errors: usize,
                warnings: usize,
                findings: Vec<Finding>,
            }
            let report = Report {
                id: &j.id,
                errors,
                warnings,
                findings,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        LintFormat::Github => {
            for f in &findings {
                let level = if f.severity == "error" {
                    "error"
                } else {
                    "warning"
                };
                let path = path.display();
                let code = f.code;
                let msg = &f.message;
                println!("::{level} file={path},title=lint/{code}::{msg}");
            }
        }
        LintFormat::Text => {
            println!("lint: {} ({} step(s))", j.id, j.steps.len());
            if findings.is_empty() {
                println!("  no findings.");
            } else {
                for f in &findings {
                    println!("  [{}] {}: {}", f.severity, f.code, f.message);
                }
            }
            println!("\n  {errors} error(s), {warnings} warning(s)");
        }
    }
    let fails = errors + if strict { warnings } else { 0 };
    Ok(if fails == 0 { 0 } else { 1 })
}

fn collect_input_refs(v: &serde_json::Value, out: &mut std::collections::HashSet<String>) {
    match v {
        serde_json::Value::Object(map) => {
            // Recognise { from: 'input', input: '<name>' } shapes.
            if let (Some(serde_json::Value::String(from)), Some(serde_json::Value::String(name))) =
                (map.get("from"), map.get("input"))
            {
                if from == "input" {
                    out.insert(name.clone());
                }
            }
            for child in map.values() {
                collect_input_refs(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_input_refs(item, out);
            }
        }
        _ => {}
    }
}

/// Collect every `{ from: 'step', stepId: '<id>' }` reference in the raw
/// JSON tree. Used by the lint to flag dangling stepId references.
fn collect_step_refs(v: &serde_json::Value, out: &mut std::collections::HashSet<String>) {
    match v {
        serde_json::Value::Object(map) => {
            if let (Some(serde_json::Value::String(from)), Some(serde_json::Value::String(name))) =
                (map.get("from"), map.get("stepId"))
            {
                if from == "step" {
                    out.insert(name.clone());
                }
            }
            for child in map.values() {
                collect_step_refs(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_step_refs(item, out);
            }
        }
        _ => {}
    }
}

/// The do→check coverage heuristic shared by `coverage` + `coverage-all`:
/// a `do` step is covered iff the step immediately after it is a `check`;
/// consecutive dos leave every earlier one bare, and a trailing do is bare.
#[derive(Debug, Default, Clone, Copy)]
struct CoverageCounts {
    total: usize,
    do_steps: usize,
    check_steps: usize,
    do_followed_by_check: usize,
    bare_do: usize,
    /// do-steps whose following check is a `{"shot": <that do's id>}` claim —
    /// the fraction of the flow covered by a pixel-diffed baseline.
    shot_covered: usize,
    /// do-steps whose following check is a `shot` OR `domshot` claim on that
    /// do — the fraction of the flow covered by ANY golden baseline
    /// (pixel or structural). `shot_covered` stays shot-only for the
    /// `shotCoverageRatio` JSON field; this is the headline number.
    golden_covered: usize,
}

impl CoverageCounts {
    fn ratio(&self) -> f64 {
        if self.do_steps == 0 {
            1.0
        } else {
            self.do_followed_by_check as f64 / self.do_steps as f64
        }
    }
    fn shot_ratio(&self) -> f64 {
        if self.do_steps == 0 {
            1.0
        } else {
            self.shot_covered as f64 / self.do_steps as f64
        }
    }
    fn golden_ratio(&self) -> f64 {
        if self.do_steps == 0 {
            1.0
        } else {
            self.golden_covered as f64 / self.do_steps as f64
        }
    }
}

fn coverage_counts(steps: &[crate::scenario::Step]) -> CoverageCounts {
    use crate::scenario::{ClaimSubject, Step};
    let mut c = CoverageCounts::default();
    let mut prev_do_id: Option<&str> = None;
    for step in steps {
        c.total += 1;
        match step {
            Step::Do { id, .. } => {
                if prev_do_id.is_some() {
                    c.bare_do += 1;
                }
                c.do_steps += 1;
                prev_do_id = Some(id.as_str());
            }
            Step::Check { claim, .. } => {
                c.check_steps += 1;
                if let Some(did) = prev_do_id.take() {
                    c.do_followed_by_check += 1;
                    if let ClaimSubject::Shot { shot, .. } = &claim.subject {
                        if shot == did {
                            c.shot_covered += 1;
                        }
                    }
                    let golden_id = match &claim.subject {
                        ClaimSubject::Shot { shot, .. } => Some(shot.as_str()),
                        ClaimSubject::Domshot { domshot, .. } => Some(domshot.as_str()),
                        _ => None,
                    };
                    if golden_id == Some(did) {
                        c.golden_covered += 1;
                    }
                }
            }
        }
    }
    if prev_do_id.is_some() {
        c.bare_do += 1;
    }
    c
}

/// A `<file>` arg that also accepts a scenario sid: when the arg is not
/// an existing file (and isn't '-' for stdin), resolve it as
/// <scenarios_root>/<arg>/scenario.json. Existing paths always win.
fn scenario_file_arg(arg: &str) -> PathBuf {
    let p = Path::new(arg);
    if arg == "-" || p.is_file() {
        return p.to_path_buf();
    }
    if let Ok(dir) = crate::paths::scenario_dir(arg) {
        let f = dir.join("scenario.json");
        if f.is_file() {
            return f;
        }
    }
    p.to_path_buf()
}

fn coverage(path: &Path, json_out: bool) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let j = load_scenario(path)?;
    let c = coverage_counts(&j.steps);
    let (total, do_steps, check_steps, do_followed_by_check, bare_do, ratio) = (
        c.total,
        c.do_steps,
        c.check_steps,
        c.do_followed_by_check,
        c.bare_do,
        c.ratio(),
    );
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report<'a> {
            id: &'a str,
            total_steps: usize,
            do_steps: usize,
            check_steps: usize,
            do_followed_by_check: usize,
            bare_do_steps: usize,
            coverage_ratio: f64,
            shot_covered_steps: usize,
            shot_coverage_ratio: f64,
            /// do steps covered by ANY golden claim (shot or domshot).
            golden_covered_steps: usize,
            golden_coverage_ratio: f64,
        }
        let report = Report {
            id: &j.id,
            total_steps: total,
            do_steps,
            check_steps,
            do_followed_by_check,
            bare_do_steps: bare_do,
            coverage_ratio: ratio,
            shot_covered_steps: c.shot_covered,
            shot_coverage_ratio: c.shot_ratio(),
            golden_covered_steps: c.golden_covered,
            golden_coverage_ratio: c.golden_ratio(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("coverage: {} ({} step(s))", j.id, total);
        println!("  do steps        : {do_steps}");
        println!("  check steps     : {check_steps}");
        println!("  do → check       : {do_followed_by_check}");
        println!("  bare do steps   : {bare_do}");
        println!("  coverage ratio  : {:.0}%", ratio * 100.0);
        println!(
            "  visual (shot)   : {} covered, {:.0}%",
            c.shot_covered,
            c.shot_ratio() * 100.0
        );
        println!(
            "  golden (any)    : {} covered, {:.0}%",
            c.golden_covered,
            c.golden_ratio() * 100.0
        );
    }
    Ok(0)
}

/// `scenario coverage-all` — the same do→check ratio rolled up across every
/// scenario under the root. Per-file `coverage` answers "is this scenario
/// thin?"; this answers "where does the suite leave do steps unchecked" —
/// rows sort worst-first so gaps surface at the top.
fn coverage_all(
    filter: Option<&str>,
    json_out: bool,
    root_override: Option<&std::path::Path>,
) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let needle = filter.map(|s| s.to_ascii_lowercase());
    let mut rows: Vec<(String, String, CoverageCounts)> = Vec::new(); // sid, intent, counts
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            let sid = match dir.file_name().map(|s| s.to_string_lossy().into_owned()) {
                Some(s) => s,
                None => continue,
            };
            if !dir.is_dir() || !dir.join("scenario.json").is_file() {
                continue;
            }
            if let Some(n) = &needle {
                if !sid.to_ascii_lowercase().contains(n) {
                    continue;
                }
            }
            let j = match load_scenario(&dir.join("scenario.json")) {
                Ok(j) => j,
                Err(_) => continue,
            };
            rows.push((sid, j.intent.clone(), coverage_counts(&j.steps)));
        }
    }
    rows.sort_by(|a, b| {
        a.2.ratio()
            .partial_cmp(&b.2.ratio())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });

    let mut agg = CoverageCounts::default();
    for (_, _, c) in &rows {
        agg.total += c.total;
        agg.do_steps += c.do_steps;
        agg.check_steps += c.check_steps;
        agg.do_followed_by_check += c.do_followed_by_check;
        agg.bare_do += c.bare_do;
        agg.shot_covered += c.shot_covered;
        agg.golden_covered += c.golden_covered;
    }

    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Row<'a> {
            sid: &'a str,
            intent: &'a str,
            total_steps: usize,
            do_steps: usize,
            do_followed_by_check: usize,
            bare_do_steps: usize,
            coverage_ratio: f64,
            shot_covered_steps: usize,
            shot_coverage_ratio: f64,
            golden_covered_steps: usize,
            golden_coverage_ratio: f64,
        }
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report<'a> {
            scenarios_root: String,
            scenarios: usize,
            total_steps: usize,
            do_steps: usize,
            check_steps: usize,
            do_followed_by_check: usize,
            bare_do_steps: usize,
            coverage_ratio: f64,
            shot_covered_steps: usize,
            shot_coverage_ratio: f64,
            golden_covered_steps: usize,
            golden_coverage_ratio: f64,
            rows: Vec<Row<'a>>,
        }
        let report = Report {
            scenarios_root: root.display().to_string(),
            scenarios: rows.len(),
            total_steps: agg.total,
            do_steps: agg.do_steps,
            check_steps: agg.check_steps,
            do_followed_by_check: agg.do_followed_by_check,
            bare_do_steps: agg.bare_do,
            coverage_ratio: agg.ratio(),
            shot_covered_steps: agg.shot_covered,
            shot_coverage_ratio: agg.shot_ratio(),
            golden_covered_steps: agg.golden_covered,
            golden_coverage_ratio: agg.golden_ratio(),
            rows: rows
                .iter()
                .map(|(sid, intent, c)| Row {
                    sid,
                    intent,
                    total_steps: c.total,
                    do_steps: c.do_steps,
                    do_followed_by_check: c.do_followed_by_check,
                    bare_do_steps: c.bare_do,
                    coverage_ratio: c.ratio(),
                    shot_covered_steps: c.shot_covered,
                    shot_coverage_ratio: c.shot_ratio(),
                    golden_covered_steps: c.golden_covered,
                    golden_coverage_ratio: c.golden_ratio(),
                })
                .collect(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(0);
    }

    println!(
        "coverage-all: {} ({} scenario(s))",
        root.display(),
        rows.len()
    );
    if rows.is_empty() {
        return Ok(0);
    }
    println!(
        "{:<24} {:>5} {:>5} {:>7} {:>5}  {:<5} {:<6} {:<6} intent",
        "sid", "steps", "do", "do→ck", "bare", "ratio", "shot%", "golden%"
    );
    for (sid, intent, c) in &rows {
        println!(
            "{:<24} {:>5} {:>5} {:>7} {:>5}  {:>4.0}% {:>4.0}% {:>4.0}% {}",
            sid,
            c.total,
            c.do_steps,
            c.do_followed_by_check,
            c.bare_do,
            c.ratio() * 100.0,
            c.shot_ratio() * 100.0,
            c.golden_ratio() * 100.0,
            intent.chars().take(40).collect::<String>()
        );
    }
    println!(
        "\nOVERALL: scenarios={} do={} do→check={} bare={} ratio={:.0}% shot={:.0}% golden={:.0}%",
        rows.len(),
        agg.do_steps,
        agg.do_followed_by_check,
        agg.bare_do,
        agg.ratio() * 100.0,
        agg.shot_ratio() * 100.0,
        agg.golden_ratio() * 100.0
    );
    Ok(0)
}

fn field(path: &Path, name: &str) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let val = v
        .get(name)
        .ok_or_else(|| anyhow!("{} has no top-level field {name:?}", path.display()))?;
    match val {
        serde_json::Value::String(s) => println!("{s}"),
        serde_json::Value::Number(n) => println!("{n}"),
        serde_json::Value::Bool(b) => println!("{b}"),
        serde_json::Value::Null => println!(),
        other => println!("{}", serde_json::to_string(other)?),
    }
    Ok(0)
}

fn step_ids(path: &Path) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let j = load_scenario(path)?;
    for step in &j.steps {
        println!("{}", step.id());
    }
    Ok(0)
}

fn intent(path: &Path) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let intent = v
        .get("intent")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("{} has no string 'intent' field", path.display()))?;
    println!("{intent}");
    Ok(0)
}

fn id(path: &Path) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("{} has no string 'id' field", path.display()))?;
    println!("{id}");
    Ok(0)
}

fn hash(path: &Path) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let h = crate::sidecar::hash_scenario_bytes(&bytes);
    println!("{h}  {}", path.display());
    Ok(0)
}

fn diff(a: &Path, b: &Path) -> Result<u8> {
    if a.as_os_str() == "-" && b.as_os_str() == "-" {
        bail!("scenario diff: cannot use '-' for both sides (stdin is one stream)");
    }
    let _a_guard = crate::io::stdin_or_path(a)?;
    let _b_guard = crate::io::stdin_or_path(b)?;
    let a = _a_guard.path();
    let b = _b_guard.path();
    let a_bytes = fs::read_to_string(a).with_context(|| format!("read {}", a.display()))?;
    let b_bytes = fs::read_to_string(b).with_context(|| format!("read {}", b.display()))?;
    // Pretty-print both as canonical JSON so cosmetic whitespace doesn't dominate.
    let a_pretty = canonicalize(&a_bytes, a)?;
    let b_pretty = canonicalize(&b_bytes, b)?;
    if a_pretty == b_pretty {
        println!("identical: {} == {}", a.display(), b.display());
        return Ok(0);
    }
    let diff = similar::TextDiff::from_lines(&a_pretty, &b_pretty)
        .unified_diff()
        .context_radius(3)
        .header(&a.display().to_string(), &b.display().to_string())
        .to_string();
    print!("{diff}");
    Ok(1)
}

fn canonicalize(body: &str, path: &Path) -> Result<String> {
    let v: serde_json::Value =
        serde_json::from_str(body).with_context(|| format!("parse {} as JSON", path.display()))?;
    Ok(serde_json::to_string_pretty(&v)?)
}

/// `<root>/<sid>/scenario.json` entries under a scenarios root, sorted.
fn root_scenario_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut targets: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path().join("scenario.json");
            if p.is_file() {
                targets.push(p);
            }
        }
    }
    targets.sort();
    targets
}

fn lint_all(
    format: LintFormat,
    strict: bool,
    only_rules: Option<&[String]>,
    exclude_rules: Option<&[String]>,
    root_override: Option<&std::path::Path>,
) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let targets = root_scenario_files(&root);

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        path: String,
        errors: usize,
        warnings: usize,
        load_error: Option<String>,
    }
    let mut rows: Vec<Row> = Vec::with_capacity(targets.len());
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut load_failures = 0usize;

    for path in &targets {
        match lint_collect(path, only_rules, exclude_rules) {
            Ok((errors, warnings)) => {
                total_errors += errors;
                total_warnings += warnings;
                rows.push(Row {
                    path: path.display().to_string(),
                    errors,
                    warnings,
                    load_error: None,
                });
            }
            Err(e) => {
                load_failures += 1;
                rows.push(Row {
                    path: path.display().to_string(),
                    errors: 0,
                    warnings: 0,
                    load_error: Some(e.to_string()),
                });
            }
        }
    }

    match format {
        LintFormat::Json => {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Report {
                scenarios_root: String,
                total: usize,
                errors: usize,
                warnings: usize,
                load_failures: usize,
                results: Vec<Row>,
            }
            let report = Report {
                scenarios_root: root.display().to_string(),
                total: rows.len(),
                errors: total_errors,
                warnings: total_warnings,
                load_failures,
                results: rows,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        LintFormat::Github => {
            // Re-run lint() per file with format=github so each finding
            // becomes its own ::error/::warning annotation line. The
            // rollup is omitted (GH Actions has no rollup concept).
            for path in &targets {
                let _ = lint(path, LintFormat::Github, strict, only_rules, exclude_rules);
            }
        }
        LintFormat::Text => {
            println!("lint-all: {}", root.display());
            if rows.is_empty() {
                println!("(no scenario.json files found)");
                return Ok(0);
            }
            for row in &rows {
                if let Some(err) = &row.load_error {
                    println!("LOAD-FAIL {}\n          {err}", row.path);
                } else {
                    let badge = if row.errors > 0 { "FAIL" } else { "OK  " };
                    println!(
                        "{badge} {}  ({} error(s), {} warning(s))",
                        row.path, row.errors, row.warnings
                    );
                }
            }
            println!(
                "\nSUMMARY: {total_errors} error(s), {total_warnings} warning(s), {load_failures} load-failure(s)"
            );
        }
    }
    let fails = total_errors + load_failures + if strict { total_warnings } else { 0 };
    Ok(if fails == 0 { 0 } else { 1 })
}

/// Light wrapper around the linter that returns only counts, used by
/// `lint-all`. Avoids re-implementing the rule set.
fn lint_collect(
    path: &Path,
    only_rules: Option<&[String]>,
    exclude_rules: Option<&[String]>,
) -> Result<(usize, usize)> {
    let exclude: std::collections::HashSet<String> = exclude_rules
        .map(|r| r.iter().cloned().collect())
        .unwrap_or_default();
    let active: Box<dyn Fn(&str) -> bool> = match only_rules {
        None => {
            let ex = exclude.clone();
            Box::new(move |c: &str| !ex.contains(c))
        }
        Some(rules) => {
            let set: std::collections::HashSet<String> = rules.iter().cloned().collect();
            let ex = exclude.clone();
            Box::new(move |c: &str| set.contains(c) && !ex.contains(c))
        }
    };
    let (findings, _j) = lint_findings(path)?;
    let errors = findings
        .iter()
        .filter(|f| f.severity == "error" && active(f.code))
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == "warning" && active(f.code))
        .count();
    Ok((errors, warnings))
}

fn count(
    filter: Option<&str>,
    json_out: bool,
    root_override: Option<&std::path::Path>,
) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let mut n = 0u32;
    let needle = filter.map(|s| s.to_ascii_lowercase());
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            if !entry.path().join("scenario.json").is_file() {
                continue;
            }
            if let Some(needle) = &needle {
                let sid = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if !sid.contains(needle) {
                    continue;
                }
            }
            n += 1;
        }
    }
    if json_out {
        let body = serde_json::json!({
            "scenariosRoot": root.display().to_string(),
            "filter": filter,
            "count": n,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        println!("{n}");
    }
    Ok(0)
}

fn latest(filter: Option<&str>, root_override: Option<&std::path::Path>) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let needle = filter.map(|s| s.to_ascii_lowercase());
    let mut best: Option<(std::time::SystemTime, String)> = None;
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            let scenario_file = dir.join("scenario.json");
            if !scenario_file.is_file() {
                continue;
            }
            let mtime = match scenario_file.metadata().and_then(|m| m.modified()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            let sid = match dir.file_name().map(|s| s.to_string_lossy().into_owned()) {
                Some(s) => s,
                None => continue,
            };
            if let Some(needle) = &needle {
                if !sid.to_ascii_lowercase().contains(needle) {
                    continue;
                }
            }
            match &best {
                None => best = Some((mtime, sid)),
                Some((ref t, _)) if mtime > *t => best = Some((mtime, sid)),
                _ => {}
            }
        }
    }
    match best {
        Some((_, sid)) => {
            println!("{sid}");
            Ok(0)
        }
        None => {
            bail!(
                "scenario latest: no scenarios under {} (have you run `start`?)",
                root.display()
            );
        }
    }
}

/// Every directory under `root` that contains a `scenario.json`,
/// as a sorted sid list. `filter` is a case-insensitive substring.
pub(crate) fn all_sids(root: &Path, filter: Option<&str>) -> Vec<String> {
    let mut sids: Vec<String> = Vec::new();
    let needle = filter.map(|s| s.to_ascii_lowercase());
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if !dir.join("scenario.json").is_file() {
                continue;
            }
            if let Some(name) = dir.file_name().map(|s| s.to_string_lossy().into_owned()) {
                if let Some(needle) = &needle {
                    if !name.to_ascii_lowercase().contains(needle) {
                        continue;
                    }
                }
                sids.push(name);
            }
        }
    }
    sids.sort();
    sids
}

fn ls(filter: Option<&str>, json_out: bool, root_override: Option<&std::path::Path>) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let sids = all_sids(&root, filter);
    if json_out {
        let body = serde_json::json!({
            "scenariosRoot": root.display().to_string(),
            "filter": filter,
            "sids": sids,
        });
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        for j in sids {
            println!("{j}");
        }
    }
    Ok(0)
}

fn validate_all(format: LintFormat, root_override: Option<&std::path::Path>) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let targets = root_scenario_files(&root);

    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        path: String,
        ok: bool,
        error: Option<String>,
    }
    let mut rows: Vec<Row> = Vec::with_capacity(targets.len());
    let mut failures = 0u32;
    for path in &targets {
        let result = fs::read(path)
            .map_err(anyhow::Error::from)
            .and_then(|b| crate::schema::validate_bytes(&b).map(|_| ()));
        let ok = result.is_ok();
        if !ok {
            failures = failures.saturating_add(1);
        }
        rows.push(Row {
            path: path.display().to_string(),
            ok,
            error: result.err().map(|e| e.to_string()),
        });
    }

    match format {
        LintFormat::Json => {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Report {
                scenarios_root: String,
                total: usize,
                ok: usize,
                failed: usize,
                results: Vec<Row>,
            }
            let report = Report {
                scenarios_root: root.display().to_string(),
                total: rows.len(),
                ok: rows.len() - failures as usize,
                failed: failures as usize,
                results: rows,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        LintFormat::Github => {
            for row in &rows {
                if let Some(err) = &row.error {
                    let msg = err.replace('\n', "; ");
                    let path = &row.path;
                    println!("::error file={path},title=schema-validate::{msg}");
                }
            }
        }
        LintFormat::Text => {
            println!("validate-all: {}", root.display());
            if rows.is_empty() {
                println!("(no scenario.json files found)");
                return Ok(0);
            }
            for row in &rows {
                if row.ok {
                    println!("OK   {}", row.path);
                } else {
                    println!("FAIL {}", row.path);
                    if let Some(e) = &row.error {
                        for line in e.lines() {
                            println!("     {line}");
                        }
                    }
                }
            }
            println!(
                "\nSUMMARY: {}/{} ok ({failures} failed)",
                rows.len() - failures as usize,
                rows.len()
            );
        }
    }
    Ok(if failures == 0 { 0 } else { 1 })
}

fn check_all(
    strict: bool,
    format: LintFormat,
    root_override: Option<&std::path::Path>,
) -> Result<u8> {
    let root = root_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(crate::paths::scenarios_root);
    let targets = root_scenario_files(&root);
    // github / json formats: defer to validate_all + lint_all so output
    // is uniform with the standalone verbs. Text mode keeps the
    // compact per-row OK/FAIL view.
    if format != LintFormat::Text {
        let v = validate_all(format, root_override)?;
        if v != 0 {
            return Ok(v);
        }
        return lint_all(format, strict, None, None, root_override);
    }
    println!(
        "check-all: {} ({} scenario(s))",
        root.display(),
        targets.len()
    );
    let mut total_fails = 0u32;
    for path in &targets {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                println!("LOAD-FAIL {}\n          {e}", path.display());
                total_fails += 1;
                continue;
            }
        };
        if let Err(e) = crate::schema::validate_bytes(&bytes) {
            println!("VALIDATE-FAIL {}\n              {e}", path.display());
            total_fails += 1;
            continue;
        }
        match lint_collect(path, None, None) {
            Ok((errors, warnings)) => {
                let gating = errors + if strict { warnings } else { 0 };
                let badge = if gating == 0 { "OK  " } else { "FAIL" };
                println!(
                    "{badge} {}  ({errors} error(s), {warnings} warning(s))",
                    path.display()
                );
                if gating != 0 {
                    total_fails += 1;
                }
            }
            Err(e) => {
                println!("LINT-FAIL {}\n          {e}", path.display());
                total_fails += 1;
            }
        }
    }
    println!(
        "\nSUMMARY: {}/{} passed, {total_fails} failed",
        targets.len() as u32 - total_fails,
        targets.len()
    );
    Ok(if total_fails == 0 { 0 } else { 1 })
}

pub(crate) fn check(path: &Path, strict: bool, format: LintFormat) -> Result<u8> {
    // For stdin ('-'), buffer once into a tempfile so the rest of
    // this function (and the validate/lint helpers it calls in text
    // mode) get a filesystem path to work with.
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    // For github / json formats, delegate to validate + lint with the
    // requested format so emissions are uniform with the standalone
    // verbs. Text mode keeps the compact 'validate OK / lint OK' view.
    if format != LintFormat::Text {
        let v = validate(path, format)?;
        if v != 0 {
            return Ok(v);
        }
        return lint(path, format, strict, None, None);
    }
    // 1) schema validate
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let validate_result = crate::schema::validate_bytes(&bytes);
    let validate_ok = validate_result.is_ok();
    if !validate_ok {
        eprintln!("validate FAIL {}", path.display());
        if let Err(e) = &validate_result {
            for line in e.to_string().lines() {
                eprintln!("  {line}");
            }
        }
        return Ok(1);
    }
    println!("validate OK {}", path.display());

    // 2) lint (errors gate; warnings gate iff strict)
    let (errors, warnings) = lint_collect(path, None, None)?;
    let fails = errors + if strict { warnings } else { 0 };
    let lint_ok = fails == 0;
    let badge = if lint_ok { "OK" } else { "FAIL" };
    println!(
        "lint     {badge} {path}  ({errors} error(s), {warnings} warning(s))",
        path = path.display()
    );
    Ok(if lint_ok { 0 } else { 1 })
}

fn validate(path: &Path, format: LintFormat) -> Result<u8> {
    let (bytes, label) = if path.as_os_str() == "-" {
        crate::io::ensure_stdin_piped("a scenario file")?;
        use std::io::Read;
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .context("read stdin")?;
        (buf, "<stdin>".to_string())
    } else {
        (
            fs::read(path).with_context(|| format!("read {}", path.display()))?,
            path.display().to_string(),
        )
    };
    let result = schema::validate_bytes(&bytes);
    match format {
        LintFormat::Json => {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Report {
                path: String,
                ok: bool,
                error: Option<String>,
            }
            let report = Report {
                path: label.clone(),
                ok: result.is_ok(),
                error: result.as_ref().err().map(|e| e.to_string()),
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(if result.is_ok() { 0 } else { 1 })
        }
        LintFormat::Github => {
            if let Err(e) = &result {
                // Collapse multi-line schema errors into a single GH
                // workflow command. Newlines aren't valid inside ::error
                // payloads; replace with semicolons.
                let msg = e.to_string().replace('\n', "; ");
                println!("::error file={label},title=schema-validate::{msg}");
            }
            Ok(if result.is_ok() { 0 } else { 1 })
        }
        LintFormat::Text => match result {
            Ok(_) => {
                println!("OK  {label}");
                Ok(0)
            }
            Err(e) => {
                eprintln!("FAIL {label}");
                eprintln!("{e}");
                Ok(1)
            }
        },
    }
}

fn load_scenario(path: &Path) -> Result<Scenario> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let value =
        schema::validate_bytes(&bytes).with_context(|| format!("validate {}", path.display()))?;
    serde_json::from_value(value).with_context(|| format!("parse {} as Scenario", path.display()))
}

fn summary(path: &Path, filter: Option<&str>, json_out: bool) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let j = load_scenario(path)?;
    let filter_lc = filter.map(|s| s.to_ascii_lowercase());
    let matches = |id: &str, intent: &str, label: &str| -> bool {
        let Some(f) = &filter_lc else { return true };
        id.to_ascii_lowercase().contains(f)
            || intent.to_ascii_lowercase().contains(f)
            || label.contains(f)
    };
    // Build a uniform per-step rows vector first; render in either mode.
    use crate::scenario::Step;
    #[derive(serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct StepRow<'a> {
        idx: usize,
        id: &'a str,
        intent: &'a str,
        kind: &'a str,
        label: String,
    }
    let mut rows: Vec<StepRow<'_>> = Vec::new();
    for (idx, step) in j.steps.iter().enumerate() {
        let (kind, label) = match step {
            Step::Do { verb, .. } => ("do", format!("do/{verb:?}").to_ascii_lowercase()),
            Step::Check { claim, .. } => (
                "check",
                format!("check/{:?}", claim.predicate).to_ascii_lowercase(),
            ),
        };
        if !matches(step.id(), step.intent(), &label) {
            continue;
        }
        rows.push(StepRow {
            idx,
            id: step.id(),
            intent: step.intent(),
            kind,
            label,
        });
    }
    if json_out {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Report<'a> {
            id: &'a str,
            intent: &'a str,
            total_steps: usize,
            matched_steps: usize,
            filter: Option<&'a str>,
            steps: Vec<StepRow<'a>>,
        }
        let report = Report {
            id: &j.id,
            intent: &j.intent,
            total_steps: j.steps.len(),
            matched_steps: rows.len(),
            filter,
            steps: rows,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(0);
    }
    println!("{}  ({} step(s))", j.id, j.steps.len());
    println!("intent: {}", j.intent);
    if let Some(env) = &j.env {
        let open_n = env.open.as_ref().map(|v| v.len()).unwrap_or(0);
        let close_n = env.close.as_ref().map(|v| v.len()).unwrap_or(0);
        if open_n + close_n > 0 {
            println!("env: open={open_n} close={close_n}");
        }
    }
    if let Some(inputs) = &j.inputs {
        if !inputs.is_empty() {
            println!("inputs: {}", inputs.len());
        }
    }
    if let Some(f) = filter {
        println!("filter: {f:?} (case-insensitive substring on id/intent/verb)");
    }
    println!();
    for row in &rows {
        let StepRow {
            idx,
            id,
            label,
            intent,
            ..
        } = row;
        println!("  {idx:>3}  {id}  {label}  \u{2014} {intent}");
    }
    if filter.is_some() {
        println!("\n  {}/{} matched", rows.len(), j.steps.len());
    }
    Ok(0)
}

fn inputs(path: &Path, json_out: bool) -> Result<u8> {
    let _guard = crate::io::stdin_or_path(path)?;
    let path = _guard.path();
    let j = load_scenario(path)?;
    let inputs = j.inputs.unwrap_or_default();
    if json_out {
        let body = serde_json::to_string_pretty(&inputs)?;
        println!("{body}");
        return Ok(0);
    }
    if inputs.is_empty() {
        println!("(no inputs declared)");
        return Ok(0);
    }
    println!("{:<24} {:<8} {:<12} default", "name", "type", "sensitive");
    println!("{}", "-".repeat(60));
    for (name, decl) in &inputs {
        let ty = format!("{:?}", decl.ty).to_ascii_lowercase();
        let sensitive = if decl.sensitive.unwrap_or(false) {
            "yes"
        } else {
            "no"
        };
        let default = decl
            .default
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "?".into()))
            .unwrap_or_else(|| "-".into());
        println!("{name:<24} {ty:<8} {sensitive:<12} {default}");
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn write(dir: &Path, body: &str) -> std::path::PathBuf {
        let p = dir.join("j.json");
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn summary_prints_per_step() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "smoke",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://x" } },
                { "id": "s1", "intent": "url ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        let code = summary(&p, None, false).unwrap();
        assert_eq!(code, 0);
        // Filter is a substring check; running it shouldn't error and
        // should still exit 0 even when nothing matches.
        assert_eq!(summary(&p, Some("nothing-matches-xyz"), false).unwrap(), 0);
        assert_eq!(summary(&p, Some("GOTO"), false).unwrap(), 0);
        // --json must also exit 0 in both filtered and unfiltered modes.
        assert_eq!(summary(&p, None, true).unwrap(), 0);
        assert_eq!(summary(&p, Some("goto"), true).unwrap(), 0);
    }

    #[test]
    fn insert_splices_a_validated_step_and_mints_a_free_id() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "smoke",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "url ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        insert(
            &p,
            "check",
            r#"{"intent":"looks right","claim":{"subject":{"url":true},"predicate":"contains","value":"/done"}}"#,
            Some("s0"),
            None,
        )
        .unwrap();
        let sc = load_scenario(&p).unwrap();
        assert_eq!(sc.steps.len(), 3);
        assert_eq!(sc.steps[1].id(), "s2");
        assert_eq!(sc.steps[1].intent(), "looks right");
        // --at 0 prepends; default appends.
        insert(
            &p,
            "do",
            r#"{"intent":"top","verb":"reload"}"#,
            None,
            Some(0),
        )
        .unwrap();
        insert(&p, "do", r#"{"intent":"tail","verb":"wait"}"#, None, None).unwrap();
        let sc = load_scenario(&p).unwrap();
        assert_eq!(sc.steps.len(), 5);
        assert_eq!(sc.steps[0].intent(), "top");
        assert_eq!(sc.steps[4].intent(), "tail");
        // Unknown --after id and out-of-range --at are errors, not writes.
        assert!(insert(
            &p,
            "do",
            r#"{"intent":"x","verb":"wait"}"#,
            Some("nope"),
            None
        )
        .is_err());
        assert!(insert(&p, "do", r#"{"intent":"x","verb":"wait"}"#, None, Some(99)).is_err());
        // An invalid draft leaves the file untouched.
        let before = fs::read_to_string(&p).unwrap();
        assert!(insert(
            &p,
            "check",
            r#"{"claim":{"subject":{"url":true},"predicate":"exists"}}"#,
            None,
            None
        )
        .is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), before);
    }

    #[test]
    fn redact_rewrites_literal_to_input_and_declares_sensitive() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r##"{
              "schema": "scenario/2", "id": "demo", "intent": "login",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "type password", "kind": "do", "verb": "type",
                  "on": { "raw": { "kind": "css", "value": "input[name=pw]" }, "reason": "test" },
                  "value": { "from": "literal", "literal": "hunter2" } },
                { "id": "s2", "intent": "note hunter2 was used", "kind": "do", "verb": "wait",
                  "params": { "label": "typed hunter2" } },
                { "id": "s3", "intent": "pw visible", "kind": "check",
                  "claim": { "subject": { "element": { "raw": { "kind": "css", "value": ".echo" }, "reason": "test" } },
                             "predicate": "contains", "value": "hunter2" } }
              ]
            }"##,
        );
        // --value: s1's whole literal upgrades to an input ref; s2's params
        // and s3's claim value get {{vars.…}} tokens.
        redact(&p, "PASS", Some("hunter2"), None, None, false).unwrap();
        let sc = load_scenario(&p).unwrap();
        let inputs = sc.inputs.clone().unwrap();
        assert_eq!(inputs["PASS"].sensitive, Some(true));
        match &sc.steps[1] {
            Step::Do { value, .. } => {
                assert!(matches!(value, Some(Value::Input { input }) if input == "PASS"))
            }
            _ => panic!(),
        }
        let body = serde_json::to_value(&sc).unwrap();
        let text = body.to_string();
        assert!(!text.contains("hunter2"), "secret must not survive: {text}");
        assert!(text.contains("{{vars.PASS}}"));
    }

    #[test]
    fn redact_by_selector_without_the_secret_on_cmdline() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "login",
              "steps": [
                { "id": "s0", "intent": "type pw", "kind": "do", "verb": "type",
                  "on": { "raw": { "kind": "css", "value": "input[type=password]" }, "reason": "test" },
                  "value": { "from": "literal", "literal": "whatever" } },
                { "id": "s1", "intent": "unrelated", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        // dry-run reports the match but does not write
        let before = fs::read_to_string(&p).unwrap();
        redact(&p, "PASS", None, Some("input[type=password]"), None, true).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), before);
        redact(&p, "PASS", None, Some("input[type=password]"), None, false).unwrap();
        let sc = load_scenario(&p).unwrap();
        match &sc.steps[0] {
            Step::Do { value, .. } => {
                assert!(matches!(value, Some(Value::Input { input }) if input == "PASS"))
            }
            _ => panic!(),
        }
        // unmatched steps untouched
        assert!(matches!(&sc.steps[1], Step::Do { value: None, .. }));
        // --step addresses the same way
        assert!(redact(&p, "X", None, None, Some("nope"), false).is_err());
        // nothing-to-do is an error, not a silent write
        assert!(redact(&p, "X", Some("absent"), None, None, false).is_err());
        // exactly one selector mode
        assert!(redact(&p, "X", Some("a"), Some("b"), None, false).is_err());
        // identifier validation
        assert!(redact(&p, "1bad", Some("whatever"), None, None, true).is_err());
        // an already-Input step can't be re-redacted
        assert!(redact(&p, "Z", None, Some("input[type=password]"), None, false).is_err());
    }

    #[test]
    fn inputs_prints_table() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "inputs": {
                "email": { "type": "string", "default": "a@b" },
                "secret": { "type": "string", "sensitive": true }
              },
              "steps": [{ "id": "s0", "intent": "go", "kind": "do", "verb": "reload" }]
            }"#,
        );
        let code = inputs(&p, false).unwrap();
        assert_eq!(code, 0);
        // --json mode also exits 0
        let code = inputs(&p, true).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn inputs_no_declaration_prints_message() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "steps": [{ "id": "s0", "intent": "go", "kind": "do", "verb": "reload" }]
            }"#,
        );
        let code = inputs(&p, false).unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn summary_errors_on_invalid_scenario() {
        let tmp = TempDir::new().unwrap();
        let p = write(tmp.path(), r#"{ "schema": "scenario/2", "id": "x" }"#);
        summary(&p, None, false).unwrap_err();
    }

    #[test]
    fn new_writes_schema_valid_scenario() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("foo.json");
        let code = new(&p, false, "https://example.com/", "smoke").unwrap();
        assert_eq!(code, 0);
        // The scaffolded file passes the schema gate.
        validate(
            &p,
            if false {
                LintFormat::Json
            } else {
                LintFormat::Text
            },
        )
        .unwrap();
        // id is derived from the file stem.
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        assert_eq!(parsed["id"], "foo");
        assert_eq!(parsed["intent"], "smoke");
        assert_eq!(parsed["env"]["open"][0]["url"], "https://example.com/");
    }

    #[test]
    fn new_refuses_to_overwrite_without_force() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("existing.json");
        fs::write(&p, "original").unwrap();
        let err = new(&p, false, "https://x/", "x").unwrap_err().to_string();
        assert!(err.contains("refusing to overwrite"));
        // File contents preserved.
        assert_eq!(fs::read_to_string(&p).unwrap(), "original");
    }

    #[test]
    fn new_force_overwrites() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("existing.json");
        fs::write(&p, "original").unwrap();
        new(&p, true, "https://x/", "x").unwrap();
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains("\"schema\": \"scenario/2\""));
    }

    fn write_har(dir: &std::path::Path, entries: serde_json::Value) -> std::path::PathBuf {
        let p = dir.join("cap.har");
        fs::write(
            &p,
            serde_json::json!({"log": {"version": "1.2", "entries": entries}}).to_string(),
        )
        .unwrap();
        p
    }

    fn har_entry(url: &str, method: &str, mime: &str, pageref: Option<&str>) -> serde_json::Value {
        let mut e = serde_json::json!({
            "request": { "method": method, "url": url },
            "response": { "status": 200, "content": { "mimeType": mime } }
        });
        if let Some(pr) = pageref {
            e["pageref"] = serde_json::Value::String(pr.to_string());
        }
        e
    }

    #[test]
    fn new_from_har_builds_nav_skeleton_per_page() {
        let tmp = TempDir::new().unwrap();
        let har = write_har(
            tmp.path(),
            serde_json::json!([
                har_entry(
                    "https://app.example.com/login",
                    "GET",
                    "text/html",
                    Some("page_1")
                ),
                har_entry(
                    "https://api.example.com/me",
                    "GET",
                    "application/json",
                    Some("page_1")
                ),
                har_entry(
                    "https://app.example.com/login/form.html",
                    "GET",
                    "text/html",
                    Some("page_1")
                ),
                har_entry(
                    "https://app.example.com/dash",
                    "GET",
                    "text/html",
                    Some("page_2")
                ),
                har_entry(
                    "https://api.example.com/items",
                    "POST",
                    "application/json",
                    Some("page_2")
                ),
            ]),
        );
        let p = tmp.path().join("s1.json");
        new_from_har(&p, &har, false, "debug repro").unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        // One nav per pageref (the fetched html fragment is skipped).
        assert_eq!(
            parsed["env"]["open"][0]["url"],
            "https://app.example.com/login"
        );
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 6, "2 navs × (goto + check + wait): {steps:?}");
        assert_eq!(steps[0]["verb"], "goto");
        assert_eq!(
            steps[0]["value"]["literal"],
            "https://app.example.com/login"
        );
        assert_eq!(steps[3]["value"]["literal"], "https://app.example.com/dash");
        assert_eq!(steps[2]["params"]["idle"], true);
        assert_eq!(parsed["intent"], "debug repro");
        // The scaffolded file passes the schema gate.
        validate(&p, LintFormat::Text).unwrap();
    }

    #[test]
    fn new_from_har_without_pageref_uses_html_responses() {
        let tmp = TempDir::new().unwrap();
        let har = write_har(
            tmp.path(),
            serde_json::json!([
                har_entry("https://a.example/", "GET", "text/html", None),
                har_entry("https://a.example/x.js", "GET", "text/javascript", None),
                har_entry("https://a.example/api", "POST", "application/json", None),
                har_entry("https://b.example/", "GET", "text/html;charset=utf-8", None),
            ]),
        );
        let p = tmp.path().join("s2.json");
        new_from_har(&p, &har, false, "").unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 6);
        assert_eq!(steps[0]["value"]["literal"], "https://a.example/");
        assert_eq!(steps[3]["value"]["literal"], "https://b.example/");
        assert!(parsed["intent"].as_str().unwrap().contains("cap.har"));
    }

    #[test]
    fn new_from_har_refuses_empty_and_docless_hars() {
        let tmp = TempDir::new().unwrap();
        let empty = write_har(tmp.path(), serde_json::json!([]));
        let err = new_from_har(&tmp.path().join("a.json"), &empty, false, "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("empty log.entries"), "{err}");
        let no_docs = write_har(
            tmp.path(),
            serde_json::json!([har_entry(
                "https://api.example/me",
                "GET",
                "application/json",
                None
            )]),
        );
        let err = new_from_har(&tmp.path().join("b.json"), &no_docs, false, "x")
            .unwrap_err()
            .to_string();
        assert!(err.contains("no HTML document navigations"), "{err}");
    }

    #[test]
    fn diff_identical_files_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let body = r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#;
        let a = write(tmp.path(), body);
        let b = tmp.path().join("b.json");
        fs::write(&b, body).unwrap();
        assert_eq!(diff(&a, &b).unwrap(), 0);
    }

    #[test]
    fn diff_different_files_returns_one() {
        let tmp = TempDir::new().unwrap();
        let a = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"a","steps":[]}"#,
        );
        let b = tmp.path().join("b.json");
        fs::write(
            &b,
            r#"{"schema":"scenario/2","id":"j","intent":"b","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(diff(&a, &b).unwrap(), 1);
    }

    #[test]
    fn hash_prints_sha256_with_path() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        // Capturing stdout cleanly here would need plumbing; just verify the
        // verb runs successfully and the file exists for the hash call.
        assert_eq!(hash(&p).unwrap(), 0);
        // Independently compute the expected hash and assert it matches what
        // sidecar::hash_scenario_bytes returns.
        let bytes = fs::read(&p).unwrap();
        let h = crate::sidecar::hash_scenario_bytes(&bytes);
        assert_eq!(h.len(), 64, "sha256 hex is 64 chars");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_errors_on_missing_file() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("nope.json");
        let err = hash(&missing).unwrap_err().to_string();
        assert!(err.contains("read "));
    }

    #[test]
    fn id_prints_scenario_id() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"my-sid","intent":"x","steps":[]}"#,
        );
        assert_eq!(id(&p).unwrap(), 0);
    }

    #[test]
    fn id_errors_when_missing_id_field() {
        let tmp = TempDir::new().unwrap();
        let p = write(tmp.path(), r#"{"schema":"scenario/2"}"#);
        let err = id(&p).unwrap_err().to_string();
        assert!(err.contains("no string 'id' field"));
    }

    #[test]
    fn intent_prints_scenario_intent() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"smoke test","steps":[]}"#,
        );
        assert_eq!(intent(&p).unwrap(), 0);
    }

    #[test]
    fn intent_errors_when_missing_intent_field() {
        let tmp = TempDir::new().unwrap();
        let p = write(tmp.path(), r#"{"schema":"scenario/2","id":"j"}"#);
        let err = intent(&p).unwrap_err().to_string();
        assert!(err.contains("no string 'intent' field"));
    }

    #[test]
    fn step_ids_prints_every_step_id() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "a", "intent": "x", "kind": "do", "verb": "reload" },
                { "id": "b", "intent": "y", "kind": "do", "verb": "reload" },
                { "id": "c", "intent": "z", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(step_ids(&p).unwrap(), 0);
    }

    #[test]
    fn step_ids_empty_when_no_steps() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        assert_eq!(step_ids(&p).unwrap(), 0);
    }

    #[test]
    fn field_prints_any_top_level_field() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        assert_eq!(field(&p, "id").unwrap(), 0);
        assert_eq!(field(&p, "schema").unwrap(), 0);
        assert_eq!(field(&p, "steps").unwrap(), 0);
        let err = field(&p, "missing").unwrap_err().to_string();
        assert!(err.contains("no top-level field"));
    }

    #[test]
    fn coverage_counts_do_check_pairs() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "url ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } },
                { "id": "s2", "intent": "reload", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        assert_eq!(coverage(&p, false).unwrap(), 0);
        assert_eq!(coverage(&p, true).unwrap(), 0);
    }

    #[test]
    fn coverage_all_rolls_up_and_sorts_worst_first() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        // thin: 2 do, 0 checks → 0%; full: 1 do + 1 check → 100%.
        let thin = tmp.path().join("thin");
        std::fs::create_dir_all(&thin).unwrap();
        fs::write(
            thin.join("scenario.json"),
            r#"{
              "schema": "scenario/2", "id": "thin", "intent": "thin one",
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        )
        .unwrap();
        let full = tmp.path().join("full");
        std::fs::create_dir_all(&full).unwrap();
        fs::write(
            full.join("scenario.json"),
            r#"{
              "schema": "scenario/2", "id": "full", "intent": "full one",
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "b", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(coverage_all(None, false, None).unwrap(), 0);
        assert_eq!(coverage_all(None, true, None).unwrap(), 0);
        // --filter narrows the rollup
        assert_eq!(coverage_all(Some("full"), true, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn scenario_file_arg_accepts_sid_and_keeps_paths() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let dir = tmp.path().join("hello");
        std::fs::create_dir_all(&dir).unwrap();
        let sc = dir.join("scenario.json");
        fs::write(&sc, "{}").unwrap();
        // sid resolves to its scenario.json
        assert_eq!(scenario_file_arg("hello"), sc);
        // existing file paths pass through untouched
        let loose = tmp.path().join("loose.json");
        fs::write(&loose, "{}").unwrap();
        assert_eq!(scenario_file_arg(loose.to_str().unwrap()), loose);
        // unknown args fall back to the literal path (read error surfaces later)
        assert_eq!(
            scenario_file_arg("no-such-sid"),
            Path::new("no-such-sid").to_path_buf()
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn coverage_counts_shared_heuristic() {
        // Direct unit coverage of the shared counter: do,do,check → 1 bare.
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "b", "kind": "do", "verb": "reload" },
                { "id": "s2", "intent": "c", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        let j = load_scenario(&p).unwrap();
        let c = coverage_counts(&j.steps);
        assert_eq!(c.total, 3);
        assert_eq!(c.do_steps, 2);
        assert_eq!(c.do_followed_by_check, 1);
        assert_eq!(c.bare_do, 1);
        assert!((c.ratio() - 0.5).abs() < 1e-9);
        assert_eq!(c.shot_covered, 0);
    }

    #[test]
    fn coverage_counts_shot_claims_on_the_following_check() {
        // do,shot-check(s0) + do,bare → shot_covered=1, do→check=1.
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "looks right", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } },
                { "id": "s2", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        let j = load_scenario(&p).unwrap();
        let c = coverage_counts(&j.steps);
        assert_eq!(c.do_steps, 2);
        assert_eq!(c.do_followed_by_check, 1);
        assert_eq!(c.bare_do, 1);
        assert_eq!(c.shot_covered, 1);
        assert!((c.shot_ratio() - 0.5).abs() < 1e-9);
        assert_eq!(c.golden_covered, 1);
        assert!((c.golden_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn coverage_counts_domshot_as_golden_not_shot() {
        // do,domshot-check(s0) + do,bare → golden_covered=1 but shot_covered=0.
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "structure holds", "kind": "check",
                  "claim": { "subject": { "domshot": "s0" }, "predicate": "matches" } },
                { "id": "s2", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        let j = load_scenario(&p).unwrap();
        let c = coverage_counts(&j.steps);
        assert_eq!(c.shot_covered, 0);
        assert_eq!(c.golden_covered, 1);
        assert!((c.golden_ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn lint_no_visual_check_accepts_domshot() {
        // A scenario whose only golden is a domshot claim must not warn —
        // strict mode fails on warnings, so a clean run proves the rule
        // counts domshot as visual coverage.
        let tmp = TempDir::new().unwrap();
        // `<sid>/scenario.json` layout + a minted baseline + env.open so the
        // only question under test is whether no-visual-check fires.
        let dir = tmp.path().join("x");
        fs::create_dir_all(dir.join("baselines")).unwrap();
        fs::write(dir.join("baselines/s0.snap.txt"), "- heading \"x\"\n").unwrap();
        let p = dir.join("scenario.json");
        fs::write(
            &p,
            r#"{
              "schema": "scenario/2", "id": "x", "intent": "y",
              "env": { "open": [{ "kind": "fresh" }] },
              "steps": [
                { "id": "s0", "intent": "a", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://example.com/" } },
                { "id": "s1", "intent": "structure holds", "kind": "check",
                  "claim": { "subject": { "domshot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 0);
    }

    #[test]
    fn lint_shot_claim_without_baseline_is_an_error() {
        // Only fires for a real `<sid>/scenario.json` layout — baselines live
        // beside it; a bare `j.json` skips the rule entirely.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("demo");
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("scenario.json");
        fs::write(
            &p,
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "looks", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(lint(&p, LintFormat::Text, false, None, None).unwrap(), 1);
        fs::create_dir_all(dir.join("baselines")).unwrap();
        fs::write(dir.join("baselines/s0.png"), b"png").unwrap();
        assert_eq!(lint(&p, LintFormat::Text, false, None, None).unwrap(), 0);
    }

    #[test]
    fn lint_upload_file_refs() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("demo");
        fs::create_dir_all(dir.join("files")).unwrap();
        let p = dir.join("scenario.json");
        fs::write(
            &p,
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "up", "kind": "do", "verb": "upload",
                  "on": "css:input[type=file]",
                  "value": { "from": "literal", "literal": "files/avatar.png" } },
                { "id": "s1", "intent": "pick", "kind": "do", "verb": "fileChooser",
                  "params": { "files": ["missing.zip", "files/avatar.png", "{{vars.p}}"] } }
              ]
            }"#,
        )
        .unwrap();
        // files/avatar.png + missing.zip don't exist yet → 3 missing
        // findings (s0 + both s1 entries); {{vars.p}} skips as templated.
        let (findings, _) = lint_findings(&p).unwrap();
        let missing: Vec<_> = findings
            .iter()
            .filter(|f| f.code == "upload-file-missing")
            .collect();
        assert_eq!(missing.len(), 3);
        fs::write(dir.join("files/avatar.png"), b"png").unwrap();
        // Packaged refs resolve now; only missing.zip still flags.
        let (findings, _) = lint_findings(&p).unwrap();
        let missing: Vec<_> = findings
            .iter()
            .filter(|f| f.code == "upload-file-missing")
            .collect();
        assert_eq!(missing.len(), 1);
        assert!(missing[0].message.contains("missing.zip"));
    }

    #[test]
    fn lint_upload_absolute_path_flags_portability() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("demo");
        fs::create_dir_all(&dir).unwrap();
        let abs = tmp.path().join("upload.bin");
        fs::write(&abs, b"x").unwrap();
        let p = dir.join("scenario.json");
        // serde escapes the path — a Windows temp dir would splice
        // raw `C:\…` backslashes into the JSON literal and fail to parse.
        fs::write(
            &p,
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": "scenario/2", "id": "demo", "intent": "x",
                "steps": [
                    { "id": "s0", "intent": "up", "kind": "do", "verb": "upload",
                      "on": "css:input[type=file]",
                      "value": { "from": "literal", "literal": abs.display().to_string() } }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        // File exists → upload-file-absolute (portability), not missing.
        let (findings, _) = lint_findings(&p).unwrap();
        assert!(findings.iter().any(|f| f.code == "upload-file-absolute"));
        assert!(!findings.iter().any(|f| f.code == "upload-file-missing"));
    }

    #[test]
    fn lint_orphan_baseline_warns_on_unclaimed_png() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("demo");
        fs::create_dir_all(dir.join("baselines")).unwrap();
        let p = dir.join("scenario.json");
        fs::write(
            &p,
            r#"{
              "schema": "scenario/2", "id": "demo", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "looks", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        )
        .unwrap();
        // Claimed baseline → clean.
        fs::write(dir.join("baselines/s0.png"), b"png").unwrap();
        assert_eq!(lint(&p, LintFormat::Text, false, None, None).unwrap(), 0);
        // s9.png has no claim → warning (exit stays 0 for warnings).
        fs::write(dir.join("baselines/s9.png"), b"png").unwrap();
        let code = lint(&p, LintFormat::Json, false, None, None).unwrap();
        assert_eq!(code, 0);
        // And the rule surfaces in --only filtering.
        let code = lint(
            &p,
            LintFormat::Json,
            true, // strict: warnings fail
            Some(&["orphan-baseline".to_string()]),
            None,
        )
        .unwrap();
        assert_eq!(code, 1);
    }

    #[test]
    fn lint_brittle_locator_flags_positional_and_generated() {
        let tmp = TempDir::new().unwrap();
        let brittle = |locator: serde_json::Value| {
            let p = write(
                tmp.path(),
                &format!(
                    r#"{{
                      "schema": "scenario/2", "id": "j", "intent": "x",
                      "steps": [
                        {{ "id": "s0", "intent": "go", "kind": "do", "verb": "click",
                          "on": {locator} }},
                        {{ "id": "s1", "intent": "ok", "kind": "check",
                          "claim": {{ "subject": {{ "url": true }}, "predicate": "exists" }} }}
                      ]
                    }}"#
                ),
            );
            // strict: warnings gate
            lint(
                &p,
                LintFormat::Text,
                true,
                Some(&["brittle-locator".to_string()]),
                None,
            )
            .unwrap()
        };
        let raw = |value: &str| serde_json::json!({ "raw": { "kind": "css", "value": value }, "reason": "r" });
        let xpath = |value: &str| serde_json::json!({ "raw": { "kind": "xpath", "value": value }, "reason": "r" });
        assert_eq!(brittle(xpath("//div[2]/span")), 1);
        assert_eq!(brittle(xpath("//ul/li[last()-1]")), 1);
        assert_eq!(brittle(xpath("//a[@href='/x']")), 0);
        assert_eq!(
            brittle(raw("div > ul > li:nth-of-type(2) > a:nth-child(1)")),
            1
        );
        assert_eq!(brittle(raw("#ember982")), 1);
        assert_eq!(brittle(raw("#a1b2c3d4")), 1);
        // Legit shapes stay clean: single positional, hash in an attribute,
        // plain ids, role locators.
        assert_eq!(brittle(raw(".todo-list li:nth-of-type(2)")), 0);
        assert_eq!(brittle(raw("a[href='#/active']")), 0);
        assert_eq!(brittle(raw("#login-button")), 0);
        assert_eq!(
            brittle(serde_json::json!({ "role": "button", "name": "Save" })),
            0
        );
    }

    #[test]
    fn lint_fixed_sleep_flags_ms_only_waits() {
        let tmp = TempDir::new().unwrap();
        let lint_one = |params: serde_json::Value| {
            let p = write(
                tmp.path(),
                &format!(
                    r#"{{
                      "schema": "scenario/2", "id": "j", "intent": "x",
                      "steps": [
                        {{ "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                          "value": {{ "from": "literal", "literal": "http://x/" }} }},
                        {{ "id": "s1", "intent": "wait", "kind": "do", "verb": "wait",
                          "params": {params} }}
                      ]
                    }}"#
                ),
            );
            lint(
                &p,
                LintFormat::Text,
                true,
                Some(&["fixed-sleep".to_string()]),
                None,
            )
            .unwrap()
        };
        assert_eq!(lint_one(serde_json::json!({ "ms": 500 })), 1);
        // Conditioned waits stay clean.
        assert_eq!(
            lint_one(serde_json::json!({ "ms": 500, "url": "*/api/*" })),
            0
        );
        assert_eq!(
            lint_one(serde_json::json!({ "ms": 500, "until": "load" })),
            0
        );
        assert_eq!(lint_one(serde_json::json!({ "ms": 500, "idle": true })), 0);
        assert_eq!(lint_one(serde_json::json!({ "url": "*/api/*" })), 0);
        assert_eq!(lint_one(serde_json::json!({ "idle": true })), 0);
    }

    #[test]
    fn lint_hardcoded_secret_flags_password_literals_only() {
        let tmp = TempDir::new().unwrap();
        let lint_one = |on: serde_json::Value, value: serde_json::Value| {
            let p = write(
                tmp.path(),
                &format!(
                    r#"{{
                      "schema": "scenario/2", "id": "x", "intent": "y",
                      "steps": [
                        {{ "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                          "value": {{ "from": "literal", "literal": "http://x/" }} }},
                        {{ "id": "s1", "intent": "type", "kind": "do", "verb": "type",
                          "on": {on}, "value": {value} }}
                      ]
                    }}"#
                ),
            );
            lint(
                &p,
                LintFormat::Text,
                true,
                Some(&["hardcoded-secret".to_string()]),
                None,
            )
            .unwrap()
        };
        let lit = |s: &str| serde_json::json!({ "from": "literal", "literal": s });
        let css =
            |s: &str| serde_json::json!({ "raw": { "kind": "css", "value": s }, "reason": "t" });
        // password-shaped locators flag
        assert_eq!(lint_one(css("input[type=password]"), lit("hunter2")), 1);
        assert_eq!(
            lint_one(
                serde_json::json!({ "role": "textbox", "name": "Password" }),
                lit("hunter2")
            ),
            1
        );
        // already-redacted steps are clean
        assert_eq!(
            lint_one(
                css("input[type=password]"),
                serde_json::json!({ "from": "input", "input": "PASS" })
            ),
            0
        );
        // non-password fields and empty literals are clean
        assert_eq!(lint_one(css("input[name=email]"), lit("hunter2")), 0);
        assert_eq!(lint_one(css("input[type=password]"), lit("")), 0);
    }

    #[test]
    fn lint_claim_value_spec_flags_from_objects() {
        let tmp = TempDir::new().unwrap();
        let lint_one = |value: serde_json::Value| {
            let p = write(
                tmp.path(),
                &format!(
                    r#"{{
                      "schema": "scenario/2", "id": "j", "intent": "x",
                      "steps": [
                        {{ "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                          "value": {{ "from": "literal", "literal": "http://x/" }} }},
                        {{ "id": "s1", "intent": "url", "kind": "check",
                          "claim": {{ "subject": {{ "url": true }}, "predicate": "contains",
                            "value": {value} }} }}
                      ]
                    }}"#
                ),
            );
            lint(
                &p,
                LintFormat::Text,
                true,
                Some(&["claim-value-spec".to_string()]),
                None,
            )
            .unwrap()
        };
        assert_eq!(
            lint_one(serde_json::json!({ "from": "literal", "literal": "x" })),
            1
        );
        assert_eq!(lint_one(serde_json::json!("x")), 0);
        assert_eq!(lint_one(serde_json::json!(42)), 0);
    }

    #[test]
    fn coverage_handles_scenario_with_no_do_steps() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        assert_eq!(coverage(&p, false).unwrap(), 0);
    }

    #[test]
    fn lint_clean_scenario_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn lint_scenario_two_do_shape_is_clean() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                  "value": { "from": "literal", "literal": "https://example.com/" } },
                { "id": "s1", "intent": "landed", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } },
                { "id": "s2", "intent": "click", "kind": "do", "verb": "click",
                  "on": { "role": "button", "name": "Continue" } },
                { "id": "s3", "intent": "button is visible", "kind": "check",
                  "claim": { "subject": { "element": { "role": "button", "name": "Continue" } }, "predicate": "isVisible" } }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
    }

    #[test]
    fn lint_duplicate_step_ids_is_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        // exit 1 because duplicate-step-id is severity=error.
        assert_eq!(
            lint(
                &p,
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn lint_undeclared_input_is_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                  "params": { "url": { "from": "input", "input": "undeclared-name" } } }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn lint_scenario_that_never_navigates_warns() {
        // `start --open <url>` drives the browser without recording anything,
        // so this shape flushes with its checks intact and no way to reach the
        // page they describe. Isolated to the one rule via --rule + --strict.
        let tmp = TempDir::new().unwrap();
        let only = vec!["no-navigation".to_string()];
        let no_nav = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "env": { "open": [ { "kind": "fresh" } ] },
              "steps": [
                { "id": "s0", "intent": "heading", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(
            lint(&no_nav, LintFormat::Json, true, Some(&only), None).unwrap(),
            1
        );

        // A recorded nav op satisfies it, same as a goto step would.
        let with_nav = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "k", "intent": "x",
              "env": { "open": [ { "kind": "fresh" },
                                 { "kind": "nav", "url": "https://example.com/users" } ] },
              "steps": [
                { "id": "s0", "intent": "heading", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(
            lint(&with_nav, LintFormat::Json, true, Some(&only), None).unwrap(),
            0
        );
    }

    #[test]
    fn lint_exclude_rule_drops_findings() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
        let excl = vec!["duplicate-step-id".to_string()];
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                Some(&excl)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn lint_rule_filter_narrows_findings() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "goto" }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
        let only = vec!["missing-locator".to_string()];
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                Some(&only),
                None
            )
            .unwrap(),
            0
        );
        let only = vec!["duplicate-step-id".to_string()];
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                Some(&only),
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn list_lint_rules_renders_both_modes() {
        assert_eq!(list_lint_rules(false).unwrap(), 0);
        assert_eq!(list_lint_rules(true).unwrap(), 0);
    }

    #[test]
    fn parse_lint_format_known_values() {
        assert!(matches!(
            parse_lint_format("text").unwrap(),
            LintFormat::Text
        ));
        assert!(matches!(
            parse_lint_format("json").unwrap(),
            LintFormat::Json
        ));
        assert!(matches!(
            parse_lint_format("github").unwrap(),
            LintFormat::Github
        ));
        let err = parse_lint_format("yaml").unwrap_err().to_string();
        assert!(err.contains("--format expects"));
    }

    #[test]
    fn lint_github_format_smoke_test() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Github, false, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_wait_without_condition_is_warning() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "env": { "open": [ { "kind": "nav", "url": "https://x", "intent": "go" } ] },
              "steps": [
                { "id": "s0", "intent": "wait", "kind": "do", "verb": "wait" },
                { "id": "s1", "intent": "ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_empty_steps_is_warning() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "env": { "open": [ { "kind": "nav", "url": "https://x", "intent": "go" } ] },
              "steps": []
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_no_checks_is_warning() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "env": { "open": [ { "kind": "nav", "url": "https://x", "intent": "go" } ] },
              "steps": [
                { "id": "s0", "intent": "reload", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_shot_claim_without_baseline_warns() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "shot", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        );
        // warning-only: exit 0 without --strict
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        // but the finding is there
        let guard = crate::io::stdin_or_path(&p).unwrap();
        let _ = guard;
        let findings_dir = tmp.path().join("baselines");
        fs::create_dir_all(&findings_dir).unwrap();
        fs::write(findings_dir.join("s0.png"), b"png").unwrap();
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
    }

    #[test]
    fn lint_shot_claim_missing_baseline_is_reported() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "shot", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        );
        // capture stdout isn't accessible — use --strict so warnings exit 1
        assert_eq!(lint(&p, LintFormat::Text, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_shot_claim_with_baseline_is_clean() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "env": { "open": [ { "kind": "nav", "url": "https://example.com/" } ] },
              "steps": [
                { "id": "s0", "intent": "shot", "kind": "check",
                  "claim": { "subject": { "shot": "s0" }, "predicate": "matches" } }
              ]
            }"#,
        );
        let baselines = tmp.path().join("baselines");
        fs::create_dir_all(&baselines).unwrap();
        fs::write(baselines.join("s0.png"), b"png").unwrap();
        assert_eq!(lint(&p, LintFormat::Text, true, None, None).unwrap(), 0);
    }

    #[test]
    fn lint_no_env_open_is_warning() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_params_on_noop_is_warning() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "reload", "kind": "do", "verb": "reload",
                  "params": { "timeoutMs": 1000 } },
                { "id": "s1", "intent": "url ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(lint(&p, LintFormat::Json, false, None, None).unwrap(), 0);
        assert_eq!(lint(&p, LintFormat::Json, true, None, None).unwrap(), 1);
    }

    #[test]
    fn lint_missing_locator_is_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "click", "kind": "do", "verb": "click" }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn lint_goto_without_url_is_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "navigate", "kind": "do", "verb": "goto" }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn lint_undeclared_step_ref_is_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "goto",
                  "params": { "url": { "from": "step", "stepId": "does-not-exist" } } }
              ]
            }"#,
        );
        assert_eq!(
            lint(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn lint_unused_input_is_warning_not_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "inputs": { "never-used": { "type": "string" } },
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        // Unused input is severity=warning → exit 0.
        assert_eq!(
            lint(
                &p,
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None
            )
            .unwrap(),
            0
        );
        // --strict promotes warnings to gating → exit 1.
        assert_eq!(
            lint(
                &p,
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                true,
                None,
                None
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn check_passes_when_validate_and_lint_ok() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "s0", "intent": "go", "kind": "do", "verb": "reload" },
                { "id": "s1", "intent": "ok", "kind": "check",
                  "claim": { "subject": { "url": true }, "predicate": "exists" } }
              ]
            }"#,
        );
        assert_eq!(check(&p, false, LintFormat::Text).unwrap(), 0);
    }

    #[test]
    fn check_fails_on_schema_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(tmp.path(), "not json");
        assert_eq!(check(&p, false, LintFormat::Text).unwrap(), 1);
    }

    #[test]
    fn check_fails_on_lint_error() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{
              "schema": "scenario/2", "id": "j", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        );
        assert_eq!(check(&p, false, LintFormat::Text).unwrap(), 1);
    }

    #[test]
    fn latest_returns_most_recently_modified() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(
            tmp.path().join("b/scenario.json"),
            r#"{"schema":"scenario/2","id":"b","intent":"updated","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(latest(None, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn count_reports_number_of_scenarios() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b", "c"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        fs::create_dir_all(tmp.path().join("no-scenario")).unwrap();
        assert_eq!(count(None, false, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn count_zero_when_empty_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path().join("empty"));
        assert_eq!(count(None, false, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn latest_errors_when_no_scenarios() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path().join("empty"));
        let err = latest(None, None).unwrap_err().to_string();
        assert!(err.contains("no scenarios"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn ls_lists_sids_alphabetically() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["zeta", "alpha", "mid"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        fs::create_dir_all(tmp.path().join("no-scenario")).unwrap();
        assert_eq!(ls(None, false, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn check_all_passes_with_clean_corpus() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!("{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[{{\"id\":\"s0\",\"intent\":\"go\",\"kind\":\"do\",\"verb\":\"reload\"}},{{\"id\":\"s1\",\"intent\":\"ok\",\"kind\":\"check\",\"claim\":{{\"subject\":{{\"url\":true}},\"predicate\":\"exists\"}}}}]}}"),
            )
            .unwrap();
        }
        assert_eq!(check_all(false, LintFormat::Text, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn check_all_fails_on_any_failing_scenario() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let a = tmp.path().join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(
            a.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"a","intent":"x","steps":[{"id":"dup","intent":"a","kind":"do","verb":"reload"},{"id":"dup","intent":"b","kind":"do","verb":"reload"}]}"#,
        )
        .unwrap();
        assert_eq!(check_all(false, LintFormat::Text, None).unwrap(), 1);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn check_all_root_override_beats_the_env_root() {
        let _g = crate::test_util::lock_env();
        let env_root = TempDir::new().unwrap();
        let flag_root = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", env_root.path());
        let a = flag_root.path().join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(
            a.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"a","intent":"x","steps":[{"id":"dup","intent":"a","kind":"do","verb":"reload"},{"id":"dup","intent":"b","kind":"do","verb":"reload"}]}"#,
        )
        .unwrap();
        assert_eq!(
            check_all(false, LintFormat::Text, Some(flag_root.path())).unwrap(),
            1
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn validate_json_ok_shape() {
        let tmp = TempDir::new().unwrap();
        let p = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        assert_eq!(
            validate(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                }
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn validate_json_fail_returns_one() {
        let tmp = TempDir::new().unwrap();
        let p = write(tmp.path(), "not json at all");
        assert_eq!(
            validate(
                &p,
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                }
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn validate_all_passes_when_all_files_ok() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        assert_eq!(
            validate_all(
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                None,
            )
            .unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn validate_all_fails_when_any_file_bad() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(
            a.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"a","intent":"x","steps":[]}"#,
        )
        .unwrap();
        fs::write(b.join("scenario.json"), "garbage not json").unwrap();
        assert_eq!(
            validate_all(
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                None,
            )
            .unwrap(),
            1
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn tag_adds_lists_and_removes() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let dir = tmp.path().join("s1");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("scenario.json");
        fs::write(
            &file,
            r#"{"schema":"scenario/2","id":"s1","intent":"x","steps":[]}"#,
        )
        .unwrap();

        // add creates + dedups + sorts
        tag(
            "s1",
            &["smoke".into(), "ci".into(), "smoke".into()],
            &[],
            false,
        )
        .unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(
            v["tags"].as_array().unwrap(),
            &vec![
                serde_json::Value::String("ci".into()),
                serde_json::Value::String("smoke".into())
            ]
        );

        // remove + list path leaves the rest
        tag("s1", &[], &["ci".into()], false).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(
            v["tags"].as_array().unwrap(),
            &vec![serde_json::Value::String("smoke".into())]
        );
        tag("s1", &[], &[], true).unwrap(); // list-only run doesn't touch the file

        // removing the last tag drops the field entirely
        tag("s1", &[], &["smoke".into()], false).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert!(v.get("tags").is_none());

        match prev {
            Some(val) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", val),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn validate_all_tolerates_empty_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path().join("empty"));
        assert_eq!(
            validate_all(
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                None,
            )
            .unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn lint_all_passes_when_every_scenario_clean() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[{{\"id\":\"s0\",\"intent\":\"go\",\"kind\":\"do\",\"verb\":\"reload\"}},{{\"id\":\"s1\",\"intent\":\"ok\",\"kind\":\"check\",\"claim\":{{\"subject\":{{\"url\":true}},\"predicate\":\"exists\"}}}}]}}"
                ),
            )
            .unwrap();
        }
        assert_eq!(
            lint_all(
                if false {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None,
                None
            )
            .unwrap(),
            0
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn lint_all_fails_when_any_scenario_has_errors() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let a = tmp.path().join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(
            a.join("scenario.json"),
            r#"{
              "schema": "scenario/2", "id": "a", "intent": "x",
              "steps": [
                { "id": "dup", "intent": "a", "kind": "do", "verb": "reload" },
                { "id": "dup", "intent": "b", "kind": "do", "verb": "reload" }
              ]
            }"#,
        )
        .unwrap();
        assert_eq!(
            lint_all(
                if true {
                    LintFormat::Json
                } else {
                    LintFormat::Text
                },
                false,
                None,
                None,
                None
            )
            .unwrap(),
            1
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn diff_canonicalises_whitespace() {
        let tmp = TempDir::new().unwrap();
        // Same logical scenario, different whitespace + key order produces
        // an identical canonicalisation — the diff verb returns 0.
        let a = write(
            tmp.path(),
            r#"{"schema":"scenario/2","id":"j","intent":"x","steps":[]}"#,
        );
        let b = tmp.path().join("b.json");
        fs::write(
            &b,
            "{\n  \"schema\": \"scenario/2\",\n  \"id\": \"j\",\n  \"intent\": \"x\",\n  \"steps\": []\n}\n",
        )
        .unwrap();
        assert_eq!(diff(&a, &b).unwrap(), 0);
    }

    #[test]
    fn rename_moves_dir_and_patches_id() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let src = tmp.path().join("old-sid");
        fs::create_dir_all(&src).unwrap();
        fs::write(
            src.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"old-sid","intent":"x","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(rename("old-sid", "new-sid").unwrap(), 0);
        assert!(!src.exists());
        let dst = tmp.path().join("new-sid");
        assert!(dst.is_dir());
        let body = fs::read_to_string(dst.join("scenario.json")).unwrap();
        assert!(body.contains("\"id\": \"new-sid\""));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn rename_refuses_to_overwrite_destination() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        let err = rename("a", "b").unwrap_err().to_string();
        assert!(err.contains("destination already exists"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn rename_rejects_invalid_new_sid() {
        let err = rename("a", "").unwrap_err().to_string();
        assert!(err.contains("non-empty"));
        let err = rename("a", "foo/bar").unwrap_err().to_string();
        assert!(err.contains("slash-free"));
    }

    #[test]
    fn copy_creates_new_dir_and_patches_id_without_touching_source() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let src = tmp.path().join("orig");
        fs::create_dir_all(src.join("replays").join("r1")).unwrap();
        fs::write(
            src.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"orig","intent":"x","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(copy("orig", "new").unwrap(), 0);
        // Source still intact, replays untouched.
        assert!(src.is_dir());
        assert!(src.join("replays").join("r1").is_dir());
        // Destination has the patched scenario.json and NO replays/.
        let dst = tmp.path().join("new");
        assert!(dst.is_dir());
        let body = fs::read_to_string(dst.join("scenario.json")).unwrap();
        assert!(body.contains("\"id\": \"new\""));
        assert!(!dst.join("replays").exists());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn copy_carries_baselines() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let src = tmp.path().join("orig");
        fs::create_dir_all(src.join("baselines")).unwrap();
        fs::create_dir_all(src.join("replays").join("r1")).unwrap();
        fs::write(
            src.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"orig","intent":"x","steps":[]}"#,
        )
        .unwrap();
        fs::write(src.join("baselines").join("s1.png"), b"golden").unwrap();
        fs::write(src.join("baselines").join("s2.png"), b"golden2").unwrap();
        assert_eq!(copy("orig", "new").unwrap(), 0);
        let dst = tmp.path().join("new");
        assert_eq!(
            fs::read(dst.join("baselines").join("s1.png")).unwrap(),
            b"golden"
        );
        assert!(dst.join("baselines").join("s2.png").is_file());
        assert!(!dst.join("replays").exists());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn copy_carries_files_and_local_inputs() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let src = tmp.path().join("orig");
        fs::create_dir_all(src.join("files")).unwrap();
        fs::write(
            src.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"orig","intent":"x","steps":[]}"#,
        )
        .unwrap();
        fs::write(src.join("files").join("avatar.png"), b"png").unwrap();
        fs::write(src.join("inputs.local.json"), b"{}").unwrap();
        assert_eq!(copy("orig", "new").unwrap(), 0);
        let dst = tmp.path().join("new");
        assert_eq!(
            fs::read(dst.join("files").join("avatar.png")).unwrap(),
            b"png"
        );
        assert!(dst.join("inputs.local.json").is_file());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn copy_refuses_to_overwrite_destination() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        for sid in ["a", "b"] {
            let d = tmp.path().join(sid);
            fs::create_dir_all(&d).unwrap();
            fs::write(
                d.join("scenario.json"),
                format!(
                    "{{\"schema\":\"scenario/2\",\"id\":\"{sid}\",\"intent\":\"x\",\"steps\":[]}}"
                ),
            )
            .unwrap();
        }
        let err = copy("a", "b").unwrap_err().to_string();
        assert!(err.contains("destination already exists"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn delete_dry_run_keeps_dir() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        fs::create_dir_all(d.join("replays").join("r1")).unwrap();
        fs::write(
            d.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"sid","intent":"x","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(delete("sid", false).unwrap(), 0);
        assert!(d.is_dir(), "dry-run must not remove the dir");
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn delete_refuses_the_active_recordings_source() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        let prev_record = std::env::var(crate::paths::RECORD_DIR_ENV).ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        std::env::set_var(crate::paths::RECORD_DIR_ENV, tmp.path().join("record"));
        let d = tmp.path().join("sid");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"sid","intent":"x","steps":[]}"#,
        )
        .unwrap();
        // A recording loaded `sid` into its buffer (source_ref) — deleting
        // it would orphan the live state and let flush recreate a zombie.
        crate::recorder_state::RecorderState::new(
            "live".into(),
            "record".into(),
            "session".into(),
            crate::recorder_state::RecorderBaseline::Fresh,
            Some("sid".into()),
            crate::browser::BrowserConnection::default(),
        )
        .save()
        .unwrap();
        let err = delete("sid", true).unwrap_err().to_string();
        assert!(err.contains("bound to the active recording"), "{err}");
        assert!(d.is_dir());
        crate::recorder_state::RecorderState::clear().unwrap();
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
        match prev_record {
            Some(v) => std::env::set_var(crate::paths::RECORD_DIR_ENV, v),
            None => std::env::remove_var(crate::paths::RECORD_DIR_ENV),
        }
    }

    #[test]
    fn delete_with_confirmed_removes_dir() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        fs::create_dir_all(d.join("replays").join("r1")).unwrap();
        fs::write(
            d.join("scenario.json"),
            r#"{"schema":"scenario/2","id":"sid","intent":"x","steps":[]}"#,
        )
        .unwrap();
        assert_eq!(delete("sid", true).unwrap(), 0);
        assert!(!d.exists());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_replays_dry_run_reports_victims() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        for i in 0..5 {
            fs::create_dir_all(d.join("replays").join(format!("2026-01-0{i}__hash{i}"))).unwrap();
        }
        assert_eq!(prune_replays("sid", 2, false, false).unwrap(), 0);
        let kept: Vec<_> = std::fs::read_dir(d.join("replays"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(kept.len(), 5);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_replays_confirmed_keeps_only_most_recent_n() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        for i in 0..5 {
            let run = d.join("replays").join(format!("2026-01-0{i}__hash{i}"));
            fs::create_dir_all(&run).unwrap();
            fs::write(run.join("events.jsonl"), "").unwrap();
        }
        assert_eq!(prune_replays("sid", 2, true, false).unwrap(), 0);
        let mut kept: Vec<String> = std::fs::read_dir(d.join("replays"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        kept.sort();
        assert_eq!(
            kept,
            vec![
                "2026-01-03__hash3".to_string(),
                "2026-01-04__hash4".to_string()
            ]
        );
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_replays_noop_when_under_keep_threshold() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        let run = d.join("replays").join("r1");
        fs::create_dir_all(&run).unwrap();
        fs::write(run.join("events.jsonl"), "").unwrap();
        assert_eq!(prune_replays("sid", 5, true, false).unwrap(), 0);
        assert!(d.join("replays").join("r1").is_dir());
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_replays_keep_failed_preserves_failures() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let d = tmp.path().join("sid");
        // 5 runs: alternating pass/fail. With --keep 2 + --keep-failed,
        // the 3 oldest would be candidates; but --keep-failed must
        // protect the failed runs so they survive.
        for (i, ec) in (0..5).zip([0, 1, 0, 1, 0]) {
            let run = d.join("replays").join(format!("2026-01-0{i}__h{i}"));
            fs::create_dir_all(&run).unwrap();
            fs::write(
                run.join("audit.json"),
                format!(
                    "{{\"schema\":\"scenario-replay-audit/v1\",\"runId\":\"h{i}\",\"scenarioId\":\"sid\",\"startedAt\":\"x\",\"exitCode\":{ec},\"scenarioContentHash\":\"deadbeef\"}}"
                ),
            )
            .unwrap();
        }
        assert_eq!(prune_replays("sid", 2, true, true).unwrap(), 0);
        // Surviving entries: at least all failed runs (h1, h3) + 2 most
        // recent (h3, h4) → set { h1, h3, h4 }.
        let kept: std::collections::HashSet<String> = fs::read_dir(d.join("replays"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(kept.contains("2026-01-01__h1"));
        assert!(kept.contains("2026-01-03__h3"));
        assert!(kept.contains("2026-01-04__h4"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_all_across_scenarios_drops_old_replays() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        // Two scenarios: a has 5 replays, b has 2.
        for j in ["a", "b"] {
            let count = if j == "a" { 5 } else { 2 };
            for i in 0..count {
                let run = tmp
                    .path()
                    .join(j)
                    .join("replays")
                    .join(format!("2026-01-0{i}__hash{i}"));
                fs::create_dir_all(&run).unwrap();
                fs::write(run.join("events.jsonl"), "").unwrap();
            }
        }
        assert_eq!(prune_all(2, true, false, None).unwrap(), 0);
        let kept_a: Vec<_> = std::fs::read_dir(tmp.path().join("a").join("replays"))
            .unwrap()
            .flatten()
            .collect();
        let kept_b: Vec<_> = std::fs::read_dir(tmp.path().join("b").join("replays"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(kept_a.len(), 2, "a: 5 → 2 after prune-all --keep 2");
        assert_eq!(kept_b.len(), 2, "b: 2 already <= 2 so untouched");
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn prune_all_tolerates_empty_root() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path().join("empty"));
        assert_eq!(prune_all(3, true, false, None).unwrap(), 0);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn delete_missing_scenario_errors() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        let err = delete("nope", true).unwrap_err().to_string();
        assert!(err.contains("not found"));
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    /// Write a scenario dir with `n` steps plus a run dir containing the
    /// given events lines; returns the scenario dir.
    fn extract_fixture(root: &Path, sid: &str, n: usize, events: &str) -> std::path::PathBuf {
        let d = root.join(sid);
        fs::create_dir_all(d.join("replays").join("r1")).unwrap();
        let steps: Vec<serde_json::Value> = (1..=n)
            .map(|i| {
                serde_json::json!({"id": format!("s{i}"), "intent": format!("step {i}"), "kind": "do", "verb": "reload"})
            })
            .collect();
        fs::write(
            d.join("scenario.json"),
            serde_json::json!({
                "schema": "scenario/2",
                "id": sid,
                "intent": "the thing",
                "env": {"open": [{"kind": "nav", "url": "https://example.com"}]},
                "steps": steps,
            })
            .to_string(),
        )
        .unwrap();
        fs::write(d.join("replays").join("latest.txt"), "r1").unwrap();
        fs::write(d.join("replays").join("r1").join("events.jsonl"), events).unwrap();
        d
    }

    #[test]
    fn extract_truncates_at_first_failed_step() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        extract_fixture(
            tmp.path(),
            "login",
            3,
            concat!(
                "{\"idx\":1,\"total\":3,\"id\":\"s1\",\"kind\":\"do:reload\",\"status\":\"running\"}\n",
                "{\"idx\":1,\"total\":3,\"id\":\"s1\",\"kind\":\"do:reload\",\"status\":\"pass\",\"ms\":10}\n",
                "{\"idx\":2,\"total\":3,\"id\":\"s2\",\"kind\":\"do:reload\",\"status\":\"running\"}\n",
                "{\"idx\":2,\"total\":3,\"id\":\"s2\",\"kind\":\"do:reload\",\"status\":\"fail\",\"ms\":50}\n",
            ),
        );
        assert_eq!(extract("login", None, None, None, false).unwrap(), 0);
        let out = tmp.path().join("login-extract").join("scenario.json");
        let sc: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(sc["id"], "login-extract");
        assert_eq!(sc["steps"].as_array().unwrap().len(), 2);
        assert_eq!(sc["steps"][1]["id"], "s2");
        assert!(sc["intent"].as_str().unwrap().contains("[extract: run r1"));
        assert!(sc["tags"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("extract")));
        // env.open survives — the extract replays standalone.
        assert_eq!(sc["env"]["open"][0]["url"], "https://example.com");
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    #[test]
    fn extract_through_cuts_at_named_step_and_requires_args() {
        let _g = crate::test_util::lock_env();
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("AGENT_QA_SCENARIOS_DIR").ok();
        std::env::set_var("AGENT_QA_SCENARIOS_DIR", tmp.path());
        extract_fixture(
            tmp.path(),
            "shop",
            4,
            "{\"idx\":1,\"id\":\"s1\",\"kind\":\"do:reload\",\"status\":\"pass\"}\n",
        );
        // No failure in the run → bails without --through.
        let err = extract("shop", None, None, None, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("no failing step"), "{err}");
        // --through cuts at the named step.
        assert_eq!(
            extract("shop", None, Some("s2"), Some("shop-prefix"), false).unwrap(),
            0
        );
        let sc: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("shop-prefix/scenario.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(sc["steps"].as_array().unwrap().len(), 2);
        match prev {
            Some(v) => std::env::set_var("AGENT_QA_SCENARIOS_DIR", v),
            None => std::env::remove_var("AGENT_QA_SCENARIOS_DIR"),
        }
    }

    /// `--help` after a subverb must print usage, not be swallowed as the
    /// `<file>` positional (`scenario lint --help` used to try reading a
    /// file literally named --help).
    #[test]
    fn help_flag_wins_over_file_positionals() {
        for args in [
            vec!["lint".to_string(), "--help".to_string()],
            vec!["validate".to_string(), "--help".to_string()],
            vec!["check".to_string(), "-h".to_string()],
            vec!["new".to_string(), "--help".to_string()],
            vec!["--help".to_string()],
        ] {
            assert_eq!(run(&args).unwrap(), 0, "args {args:?}");
        }
    }
}
