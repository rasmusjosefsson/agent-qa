//! End-to-end generic recorder contract test.

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use tempfile::TempDir;

    use crate::browser;
    use crate::paths;
    use crate::test_util::lock_env;

    fn install_fake_browser(dir: &Path, log: &Path) {
        let body = format!(
            "#!/bin/sh\necho \"cdp=$AGENT_BROWSER_CDP pin=$AGENT_BROWSER_PIN_TAB\" >> '{}'\necho \"args=$*\" >> '{}'\nexit 0\n",
            log.display(), log.display()
        );
        let binary = dir.join("agent-browser");
        fs::write(&binary, body).unwrap();
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
        std::env::set_var(browser::BIN_ENV, binary);
        browser::_reset_bin_cache_for_tests();
    }

    #[test]
    fn recording_flushes_checks_and_replays_the_same_contract() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        let record_root = tmp.path().join("record");
        let log = tmp.path().join("agent-browser.log");
        std::env::set_var(paths::SCENARIOS_DIR_ENV, tmp.path());
        std::env::set_var(paths::RECORD_DIR_ENV, &record_root);
        std::env::set_var("AGENT_BROWSER_CDP", "9223");
        std::env::set_var("AGENT_BROWSER_PIN_TAB", "1");
        std::env::set_var("AGENT_QA_RECORD_SKIP_SIDECARS", "1");
        install_fake_browser(tmp.path(), &log);

        crate::start::run(&[
            "record a generic flow".into(),
            "--session".into(),
            "recording".into(),
            "--source-ref=change:123".into(),
        ])
        .unwrap();
        crate::record_setup::run(&[
            r#"{"kind":"flag","name":"example-flag","enabled":true}"#.into()
        ])
        .unwrap();
        crate::record_step::run(&[
            "do".into(),
            r#"{"intent":"open users","verb":"goto","value":{"from":"literal","literal":"https://example.com/users"}}"#.into(),
        ])
        .unwrap();
        crate::record_step::run(&[
            "do".into(),
            r#"{"intent":"open editor","verb":"click","on":{"role":"button","name":"Edit user"}}"#
                .into(),
        ])
        .unwrap();
        crate::record_step::run(&[
            "check".into(),
            r#"{"intent":"the page has a URL","claim":{"subject":{"url":true},"predicate":"exists"}}"#.into(),
        ])
        .unwrap();
        crate::flush::run(&[]).unwrap();

        let sid = fs::read_to_string(paths::record_last_sid_file()).unwrap();
        let scenario_file = paths::scenario_dir(sid.trim())
            .unwrap()
            .join("scenario.json");
        crate::scenario_cli::run(&["check".into(), scenario_file.display().to_string()]).unwrap();
        crate::runner::cli(&[
            scenario_file.display().to_string(),
            "--session".into(),
            "replay".into(),
            "--no-sidecars".into(),
        ])
        .unwrap();

        let scenario: serde_json::Value =
            serde_json::from_slice(&fs::read(&scenario_file).unwrap()).unwrap();
        assert_eq!(scenario["producedBy"]["producer"], "agent-recorder");
        assert_eq!(scenario["producedBy"]["sourceRef"], "change:123");
        assert_eq!(scenario["env"]["open"][0]["kind"], "fresh");
        assert_eq!(scenario["env"]["open"][1]["kind"], "flag");
        assert_eq!(scenario["steps"][0]["kind"], "do");
        assert_eq!(scenario["steps"][1]["on"]["role"], "button");
        assert_eq!(scenario["steps"][2]["kind"], "check");
        assert!(scenario.get("browser").is_none());
        assert!(!record_root.join("recorder-state.json").exists());

        let commands = fs::read_to_string(&log).unwrap();
        let connection_lines: Vec<&str> = commands
            .lines()
            .filter(|line| line.starts_with("cdp="))
            .collect();
        assert!(!connection_lines.is_empty(), "got: {commands}");
        for line in connection_lines {
            assert_eq!(line, "cdp=9223 pin=1", "got: {line}");
        }
        assert!(commands.contains("--session replay open https://example.com/users"));
        assert!(commands.contains("--session replay find role button click --name Edit user"));

        std::env::remove_var(paths::SCENARIOS_DIR_ENV);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_BROWSER_CDP");
        std::env::remove_var("AGENT_BROWSER_PIN_TAB");
        std::env::remove_var("AGENT_QA_RECORD_SKIP_SIDECARS");
        std::env::remove_var(browser::BIN_ENV);
        browser::_reset_bin_cache_for_tests();
    }
}
