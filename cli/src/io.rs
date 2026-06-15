//! Tiny IO helper: when a file argument is `-`, buffer stdin into a
//! NamedTempFile and return a path-like that points at it. The tempfile
//! is dropped (and the file unlinked) when the returned guard goes out
//! of scope. Used by verbs that want stdin support without rewriting
//! their internals to take bytes.
//!
//! Usage:
//! ```ignore
//! let guard = io::stdin_or_path(path)?;
//! let bytes = fs::read(guard.path())?;
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Either a borrowed caller path (the common case) or a tempfile-backed
/// path that owns its lifetime. Use `.path()` to get a `&Path`.
pub enum StdinOrPath<'a> {
    Borrowed(&'a Path),
    /// Tempfile held alive for the duration of this enum's lifetime.
    Stdin {
        _tmp: tempfile::NamedTempFile,
        buf: PathBuf,
    },
}

impl<'a> StdinOrPath<'a> {
    pub fn path(&self) -> &Path {
        match self {
            StdinOrPath::Borrowed(p) => p,
            StdinOrPath::Stdin { buf, .. } => buf.as_path(),
        }
    }
}

/// Wrap a caller-supplied path so a literal `-` is replaced with a
/// tempfile that holds buffered stdin. Other paths are returned
/// borrowed, unchanged.
pub fn stdin_or_path(path: &Path) -> Result<StdinOrPath<'_>> {
    if path.as_os_str() != "-" {
        return Ok(StdinOrPath::Borrowed(path));
    }
    use std::io::{Read, Write};
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .context("read stdin")?;
    let tmp = tempfile::NamedTempFile::new().context("open stdin tempfile")?;
    std::fs::File::create(tmp.path())
        .and_then(|mut f| f.write_all(&buf))
        .context("write stdin tempfile")?;
    let buf = tmp.path().to_path_buf();
    Ok(StdinOrPath::Stdin { _tmp: tmp, buf })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn borrowed_when_path_not_dash() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.json");
        fs::write(&p, b"hello").unwrap();
        let guard = stdin_or_path(&p).unwrap();
        assert_eq!(guard.path(), p);
        assert_eq!(fs::read(guard.path()).unwrap(), b"hello");
    }

    // The dash branch reads from stdin, which is hard to exercise in a
    // unit test without process plumbing. The four verbs that already
    // use the same pattern (validate, lint, check, summary) exercise
    // it end-to-end in their integration runs.
}
