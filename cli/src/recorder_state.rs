//! Typed state for one in-flight recording.

use std::fs;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::browser::BrowserConnection;
use crate::paths;
use crate::scenario::{EnvOp, Step};
use crate::sidecar::atomic_write_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecorderBaseline {
    Fresh,
    Profile { name: String },
    KeepSession,
}

impl RecorderBaseline {
    pub(crate) fn from_start(profile: Option<&str>, keep_session: bool) -> Self {
        if keep_session {
            Self::KeepSession
        } else if let Some(name) = profile.filter(|name| !name.is_empty()) {
            Self::Profile {
                name: name.to_string(),
            }
        } else {
            Self::Fresh
        }
    }

    fn env_open(&self) -> Vec<EnvOp> {
        match self {
            Self::Fresh => vec![EnvOp::Fresh {
                intent: None,
                policy: None,
            }],
            Self::Profile { name } => vec![EnvOp::UseProfile {
                intent: None,
                name: name.clone(),
                policy: None,
            }],
            Self::KeepSession => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecorderState {
    pub(crate) sid: String,
    pub(crate) intent: String,
    pub(crate) session: String,
    pub(crate) baseline: RecorderBaseline,
    pub(crate) env_open: Vec<EnvOp>,
    pub(crate) source_ref: Option<String>,
    pub(crate) started_at: String,
    pub(crate) browser: BrowserConnection,
    pub(crate) steps: Vec<Step>,
    /// While paused, record append paths (`record-step`, smart-click,
    /// fill-unique, the workbench auto-record hook) drop steps instead of
    /// writing them. Missing on state files written before this field existed.
    #[serde(default)]
    pub(crate) paused: bool,
    /// Set by `buffer load`: the scenario document the steps were loaded
    /// from. `flush` merges the fields it doesn't own (id, tags, inputs,
    /// templates, env.close) back so a load → flush round-trip doesn't drop
    /// them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) original: Option<serde_json::Value>,
}

impl RecorderState {
    pub(crate) fn new(
        sid: String,
        intent: String,
        session: String,
        baseline: RecorderBaseline,
        source_ref: Option<String>,
        browser: BrowserConnection,
    ) -> Self {
        let env_open = baseline.env_open();
        Self {
            sid,
            intent,
            session,
            baseline,
            env_open,
            source_ref,
            started_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            browser,
            steps: Vec::new(),
            paused: false,
            original: None,
        }
    }

    pub(crate) fn load_active() -> Result<Self> {
        Self::try_load_active()?.ok_or_else(|| {
            anyhow!(
                "no active recording at {} (was `start` run?)",
                paths::record_state_file().display()
            )
        })
    }

    pub(crate) fn try_load_active() -> Result<Option<Self>> {
        let path = paths::record_state_file();
        let state: Self = match fs::read_to_string(&path) {
            Ok(body) => {
                serde_json::from_str(&body).with_context(|| format!("parse {}", path.display()))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        crate::browser::set_connection(&state.browser);
        Ok(Some(state))
    }

    pub(crate) fn save(&self) -> Result<()> {
        let path = paths::record_state_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
        }
        let mut body = serde_json::to_vec_pretty(self)?;
        body.push(b'\n');
        atomic_write_file(&path, &body)
    }

    /// First write of the active-state file, created atomically — the
    /// atomic answer to "no recording is active". Two `record start`s
    /// racing past `try_load_active` both see `None`, but only the first
    /// `create_new` succeeds; the loser's start refuses instead of
    /// silently orphaning the winner's in-flight recording.
    pub(crate) fn claim_active(&self) -> Result<()> {
        let path = paths::record_state_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir -p {}", parent.display()))?;
        }
        let mut body = serde_json::to_vec_pretty(self)?;
        body.push(b'\n');
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    anyhow!(
                        "another recording claimed {} just now — `record status` shows which; `start --force` abandons it",
                        path.display()
                    )
                } else {
                    anyhow!(e).context(format!("create {}", path.display()))
                }
            })?;
        use std::io::Write;
        f.write_all(&body)
            .with_context(|| format!("write {}", path.display()))
    }

    pub(crate) fn clear() -> Result<()> {
        let path = paths::record_state_file();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("remove {}", path.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::lock_env;
    use tempfile::TempDir;

    #[test]
    fn state_round_trips_complete_recording() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path());
        let mut state = RecorderState::new(
            "s1".into(),
            "record".into(),
            "session".into(),
            RecorderBaseline::Fresh,
            Some("opaque-ref".into()),
            BrowserConnection {
                cdp: Some("9223".into()),
                pin_tab: Some(true),
            },
        );
        state.steps.push(
            serde_json::from_value(serde_json::json!({
                "id": "s0", "intent": "reload", "kind": "do", "verb": "reload"
            }))
            .unwrap(),
        );
        state.save().unwrap();
        let loaded = RecorderState::load_active().unwrap();

        assert_eq!(loaded.browser, state.browser);
        assert_eq!(loaded.source_ref.as_deref(), Some("opaque-ref"));
        assert_eq!(loaded.env_open.len(), 1);
        assert_eq!(loaded.steps.len(), 1);
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_BROWSER_CDP");
        std::env::remove_var("AGENT_BROWSER_PIN_TAB");
    }

    #[test]
    fn claim_active_refuses_a_second_claim() {
        let _guard = lock_env();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(paths::RECORD_DIR_ENV, tmp.path());
        let mk = |sid: &str| {
            RecorderState::new(
                sid.into(),
                "record".into(),
                "session".into(),
                RecorderBaseline::Fresh,
                None,
                BrowserConnection::default(),
            )
        };
        mk("s1").claim_active().unwrap();
        let err = mk("s2").claim_active().unwrap_err().to_string();
        assert!(err.contains("another recording claimed"), "{err}");
        // save() still overwrites freely — only the claim is exclusive.
        mk("s2").save().unwrap();
        RecorderState::clear().unwrap();
        std::env::remove_var(paths::RECORD_DIR_ENV);
        std::env::remove_var("AGENT_BROWSER_CDP");
        std::env::remove_var("AGENT_BROWSER_PIN_TAB");
    }
}
