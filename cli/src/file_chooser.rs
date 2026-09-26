//! Native file-chooser interception for replay.
//!
//! Clicking an `<input type="file">` (directly, via a `<label for>`, or via
//! `input.click()` / `showOpenFilePicker()`) opens the OS picker — which a
//! replayed scenario cannot drive and (headless) leaves hanging. This module
//! installs an in-page hook that intercepts every path into the chooser and
//! resolves it with scenario-provided files instead:
//!
//!   1. `input.click()` / `input.showPicker()` on a file input — patched;
//!      when the chooser is armed the input is captured instead of executed.
//!   2. A real (trusted-path) click on a file input or its `<label for>` —
//!      a capture-phase listener calls `preventDefault()`/`stopPropagation()`
//!      before the browser's default action opens the OS dialog.
//!   3. `window.showOpenFilePicker()` — patched; resolves to synthetic
//!      `FileSystemFileHandle`-shaped objects (`{ kind:'file', name, getFile }`).
//!
//! The replay flow is `do/verb=fileChooser` (arm + payload files) placed BEFORE
//! the click that would open the picker. A chooser already pending at arm time
//! is resolved immediately. Because files arrive as real `File` objects on
//! `input.files` (or `getFile()` handles) with `input`/`change` dispatched,
//! the page's own handlers run exactly as if the user picked them.

/// Injected once per document. Idempotent via `window.__aqFileChooser`.
pub fn build_install_hook() -> String {
    r#"(() => {
  if (window.__aqFileChooser) return true;
  const C = (window.__aqFileChooser = {
    armed: false,
    files: null,       // [{name, type, b64}]
    pending: null,     // {kind:'input', input} | {kind:'picker', resolve, opts}
  });

  const toFiles = (payload) => payload.map((f) => {
    const bin = atob(f.b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new File([bytes], f.name, { type: f.type || 'application/octet-stream' });
  });

  const dispatch = (input) => {
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  };

  C.resolve = () => {
    const files = C.files ? toFiles(C.files) : [];
    const pending = C.pending;
    C.pending = null;
    if (!pending) return 'no-pending';
    if (pending.kind === 'input') {
      const dt = new DataTransfer();
      for (const f of files) dt.items.add(f);
      pending.input.files = dt.files;
      dispatch(pending.input);
      return 'input';
    }
    // showOpenFilePicker — resolve with handle-shaped objects the app's
    // handler can call getFile() on.
    pending.resolve(files.map((f) => ({
      kind: 'file',
      name: f.name,
      getFile: async () => f,
    })));
    return 'picker';
  };

  const capture = (input) => {
    C.pending = { kind: 'input', input };
    if (C.armed) C.resolve();
  };

  // Path 1: programmatic input.click() / input.showPicker()
  const origClick = HTMLInputElement.prototype.click;
  HTMLInputElement.prototype.click = function () {
    if (this && this.type === 'file' && (C.armed || C.pending == null)) {
      capture(this);
      return;
    }
    return origClick.apply(this, arguments);
  };
  if (HTMLInputElement.prototype.showPicker) {
    const origShow = HTMLInputElement.prototype.showPicker;
    HTMLInputElement.prototype.showPicker = function () {
      if (this && this.type === 'file') {
        capture(this);
        return;
      }
      return origShow.apply(this, arguments);
    };
  }

  // Path 2: trusted-path click on the input itself or its <label for> —
  // the chooser is the default action; preventDefault swallows it.
  document.addEventListener('click', (e) => {
    const t = e.target;
    if (!(t instanceof Element)) return;
    const input = t.closest('input[type="file"]');
    if (input) { capture(input); e.preventDefault(); e.stopPropagation(); return; }
    const label = t.closest('label[for]');
    if (label) {
      const target = document.getElementById(label.getAttribute('for'));
      if (target && target.type === 'file') {
        capture(target); e.preventDefault(); e.stopPropagation();
      }
    }
  }, true);

  // Path 3: window.showOpenFilePicker — keep the promise pending until the
  // chooser is armed+resolved, so a fileChooser step can run after the click.
  if (window.showOpenFilePicker) {
    const origPicker = window.showOpenFilePicker.bind(window);
    window.showOpenFilePicker = (opts) => {
      if (C.armed) {
        const files = toFiles(C.files || []);
        return Promise.resolve(files.map((f) => ({
          kind: 'file', name: f.name, getFile: async () => f,
        })));
      }
      return new Promise((resolve) => {
        C.pending = { kind: 'picker', resolve, opts };
      });
    };
    // Restore for completeness if something disarms (not exposed today).
    C._origPicker = origPicker;
  }
  return true;
})()"#
        .to_string()
}

/// Arm the chooser with a file payload; resolves an already-pending chooser.
/// `files` is a JSON array of `{name, type, b64}`.
pub fn build_arm(files_json: &str) -> String {
    format!(
        r#"(() => {{
  const C = window.__aqFileChooser;
  if (!C) throw new Error('fileChooser hook not installed');
  C.files = {files_json};
  C.armed = true;
  if (C.pending) return 'resolved:' + C.resolve();
  return 'armed';
}})()"#
    )
}

/// Read chooser state for tests/diagnostics.
#[allow(dead_code)]
pub fn build_state_probe() -> String {
    r#"(() => {
  const C = window.__aqFileChooser;
  return C ? { armed: C.armed, pending: !!C.pending, files: (C.files || []).length } : null;
})()"#
        .to_string()
}

/// Encode a file payload entry ({name, type, b64}) for the arm call.
pub fn file_entry(name: &str, mime: &str, bytes: &[u8]) -> serde_json::Value {
    use base64::Engine;
    serde_json::json!({
        "name": name,
        "type": mime,
        "b64": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_hook_patches_all_three_entry_points() {
        let js = build_install_hook();
        for marker in [
            "__aqFileChooser",
            "HTMLInputElement.prototype.click",
            "showPicker",
            "addEventListener('click'",
            "preventDefault",
            "showOpenFilePicker",
            "getFile",
        ] {
            assert!(js.contains(marker), "missing {marker}");
        }
    }

    #[test]
    fn arm_sets_payload_and_resolves_pending() {
        let js = build_arm(r#"[{"name":"a.png","type":"image/png","b64":"QQ=="}]"#);
        assert!(js.contains("C.armed = true"));
        assert!(js.contains("a.png"));
        assert!(js.contains("C.resolve()"));
    }

    #[test]
    fn file_entry_base64_encodes() {
        let e = file_entry("x.txt", "text/plain", b"hi");
        assert_eq!(e["name"], "x.txt");
        assert_eq!(e["b64"], "aGk=");
    }
}
