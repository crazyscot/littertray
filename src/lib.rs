// Copyright (c) 2020 Sergio Benitez, (c) 2025 Ross Younger
//! Lightweight sandboxing for tests that write to the filesystem
//!
//!
//! This is a derivative work of
//! [`figment::Jail`](https://docs.rs/figment/latest/figment/struct.Jail.html)
//! but simpler (no environment variables), and it supports async closures.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(all(feature = "async", not(async_supported)))]
compile_error!(r#"The "async" feature requires Rust compiler version 1.85 or later."#);

use std::fs::{self, File};
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use thiserror::Error;

/// The result type used internally by [`LitterTray`].
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// The error type used by [`LitterTray`].
///
/// Normally, Errors generated within the `LitterTray` are coerced to [`anyhow::Error`] by the closure.
/// However you can arrange to send these outside of the tray and reason about them if this is useful.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// An I/O error occurred. Refer to the contained [`io::Error`](std::io::Error) for more details.
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    /// The requested path is outside the [`LitterTray`] sandbox.
    /// (This is only returned by [`LitterTray`] methods; it makes no attempt to intercept filesystem calls.)
    #[error("requested path is outside of the sandbox")]
    Uncontained(PathBuf),
}

/// Lightweight filesystem sandbox
///
/// This is little more than a convenience wrapper to
/// [`tempdir::TempDir`](https://docs.rs/tempdir/latest/tempdir/struct.TempDir.html).
/// You provide a closure, which is executed in a fresh sandbox (temporary directory);
/// there are convenience methods to create files, directories and so forth.
///
/// The process changes directory into the sandbox during execution, but is not well constrained.
///
/// On drop, the temporary directory is automatically cleaned up.
///
/// # Safety / reliability
///
/// While this crate contains no _unsafe_ Rust, it is not without limitation.
/// There is a global lock to prevent tests from conflicting when run in parallel
/// (which is cargo's default behaviour).
///
/// <div class="warning">
///
/// Exercise caution when using this crate within `rusty_fork_test` or similar mechanisms!
///
/// </div>
///
/// There is a race condition:
/// - if tests run in a child process ([`rusty_fork`](https://docs.rs/rusty-fork/latest/rusty_fork/)); _and_
/// - if any `LitterTray` tests run _not_ in a child process.
///
/// In this case, the tests which run in a child process sometimes start up in an invalid state
/// (current working directory invalid/nonexistent).
///
/// The solution is to either:
/// - not use `rusty_fork`; _or_
/// - _always_ run `LitterTray` tests within `rusty_fork`.
///
/// Tests using `LitterTray` are always safe from each other, due to the global lock.
///
#[derive(Debug)]
pub struct LitterTray {
    canonical_dir: PathBuf,
    _dir: TempDir,
    saved_cwd: PathBuf,
}

#[cfg(not(feature = "async"))]
/// Synchronisation primitives when the `async` feature is NOT enabled.
mod sync {
    use std::sync::LazyLock;
    use std::sync::Mutex;

    /// Locks the global lock synchronously
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure.
    /// See [`Mutex#poisoning`](std::sync::Mutex#poisoning).
    #[allow(clippy::module_name_repetitions)]
    pub fn global_lock_sync() -> std::sync::MutexGuard<'static, ()> {
        G_LOCK.lock().unwrap()
    }
    static G_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
}
#[cfg(not(feature = "async"))]
pub use sync::global_lock_sync;

#[cfg(feature = "async")]
/// Synchronisation primitives when the `async` feature is enabled.
mod r#async {
    use std::sync::LazyLock;
    use tokio::sync::Mutex;

    /// Locks the global lock synchronously (for use by non-async functions).
    /// Async functions should use [`global_lock_async`].
    pub fn global_lock_sync() -> tokio::sync::MutexGuard<'static, ()> {
        G_LOCK.blocking_lock()
    }
    /// Locks the global lock in an async manner (for use by async functions).
    /// Non-async functions should use [`global_lock_sync`].
    pub async fn global_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        G_LOCK.lock().await
    }
    static G_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
}

#[cfg(feature = "async")]
pub use r#async::{global_lock_async, global_lock_sync};

impl LitterTray {
    /// Runs a closure in a sandbox, passing the sandbox to the closure.
    ///
    /// # Closure signature
    ///
    /// `FnOnce(&mut LitterTray) -> anyhow::Result<R>` for any R
    ///
    /// That is to say, it's a closure that takes one argument (the `LitterTray` itself), and can return any Result.
    /// (If you have nothing to return, return `Ok(())`, or see [`run()`](#method.run).)
    ///
    /// # Returns
    ///
    /// The return value of the closure.
    ///
    /// # Type parameters
    ///
    /// - F: Closure function. This is usually inferred.
    /// - R: The Result type returned by the closure on success. This is usually inferred.
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure.
    /// (This can only happen with the `async` feature is _not_ activated.
    /// See [`Mutex#poisoning`](std::sync::Mutex#poisoning)).
    ///
    /// # Example
    ///
    /// ```rust
    /// use littertray::LitterTray;
    ///
    /// let result = LitterTray::try_with(|tray| {
    ///   let _ = tray.create_text("test.txt", "Hello, world!")?;
    ///   assert_eq!(std::fs::read_to_string("test.txt")?, "Hello, world!");
    ///   Ok(42)
    /// }).unwrap();
    /// ```
    #[track_caller]
    pub fn try_with<F, R>(f: F) -> anyhow::Result<R>
    where
        F: FnOnce(&mut LitterTray) -> anyhow::Result<R>,
    {
        let dir = TempDir::new()?;
        let guard = global_lock_sync();
        let mut tray = LitterTray {
            canonical_dir: dir.path().canonicalize()?,
            _dir: dir,
            saved_cwd: std::env::current_dir()?,
        };
        std::env::set_current_dir(tray.directory())?;
        let outcome = f(&mut tray);
        drop(tray); // Force cleanup & reset of working directory before we release the lock
        drop(guard);
        outcome
    }

    /// Runs a closure in a sandbox, passing the sandbox to the closure.
    ///
    /// This is a convenience wrapper for [`LitterTray::try_with`] which returns nothing.
    /// The closure must return nothing.
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure.
    /// (This can only happen with the `async` feature is _not_ activated.
    /// See [`Mutex#poisoning`](std::sync::Mutex#poisoning)).
    ///
    /// # Example
    ///
    /// ```
    /// use littertray::LitterTray;
    ///
    /// let result = LitterTray::run(|tray| {
    ///   let _ = tray.create_text("test.txt", "Hello, world!").unwrap();
    ///   assert_eq!(std::fs::read_to_string("test.txt").unwrap(), "Hello, world!");
    /// });
    /// ```
    pub fn run<F: FnOnce(&mut LitterTray)>(f: F) {
        let _ = Self::try_with(|tray| {
            f(tray);
            Ok(())
        });
    }

    /// Runs an async closure in a sandbox, passing the sandbox to the closure.
    ///
    /// This is the same as [`try_with()`](#method.try_with), but async.
    #[cfg(all(feature = "async", async_supported))]
    pub async fn try_with_async<F, R>(f: F) -> anyhow::Result<R>
    where
        F: AsyncFnOnce(&mut LitterTray) -> anyhow::Result<R>,
    {
        let dir = TempDir::new()?;
        let guard = global_lock_async().await;
        let mut tray = LitterTray {
            canonical_dir: dir.path().canonicalize()?,
            _dir: dir,
            saved_cwd: std::env::current_dir()?,
        };
        std::env::set_current_dir(tray.directory())?;
        let outcome = f(&mut tray).await;
        drop(tray); // Force cleanup & reset of working directory before we release the lock
        drop(guard);
        outcome
    }

    /// Returns the absolute path to the temporary directory that is this sandbox.
    /// This directory will be removed on drop.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.canonical_dir
    }

    fn safe_path_within_tray<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let path = dedot(path);
        if path.is_absolute() {
            if path.starts_with(self.directory()) {
                return Ok(path);
            }
            return Err(Error::Uncontained(path));
        }
        Ok(path)
    }

    /// Creates a binary file within the sandbox from the provided contents.
    ///
    /// The given path must either be a relative filename,
    /// or an absolute path within the sandbox (see [`LitterTray::directory()`]).
    pub fn create_binary<P: AsRef<Path>>(&self, path: P, bytes: &[u8]) -> Result<File> {
        let path = self.safe_path_within_tray(path)?;
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(bytes)?;
        Ok(writer
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?)
    }

    /// Creates a text file within the sandbox from the provided contents.
    ///
    /// The given path must either be a relative filename,
    /// or an absolute path within the sandbox (see [`LitterTray::directory()`]).
    pub fn create_text<P: AsRef<Path>>(&self, path: P, contents: &str) -> Result<File> {
        self.create_binary(path, contents.as_bytes())
    }

    /// Creates a directory within the sandbox.
    ///
    /// The given path must either be a relative filename,
    /// or an absolute path within the sandbox (see [`LitterTray::directory()`]).
    ///
    /// ```
    /// use littertray::LitterTray;
    ///
    /// let result = LitterTray::run(|tray| {
    ///   let _ = tray.make_dir("mydir").unwrap();
    ///   assert!(std::fs::exists("mydir").unwrap());
    /// });
    /// ```
    pub fn make_dir<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let path = self.safe_path_within_tray(path)?;
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    #[cfg(unix)]
    /// Creates a symbolic link within the sandbox.
    /// Returns the path to the new symlink.
    ///
    /// *This method is only available on Unix platforms.*
    pub fn make_symlink<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        original: P,
        link: Q,
    ) -> Result<PathBuf> {
        let path_orig = self.safe_path_within_tray(original)?;
        let path_link = self.safe_path_within_tray(link)?;
        std::os::unix::fs::symlink(path_orig, &path_link)?;
        Ok(path_link)
    }
}

impl Drop for LitterTray {
    /// On drop, `LitterTray`:
    /// - Changes the process's working directory to whatever it was on entry
    /// - Cleans up the sandbox directory
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.saved_cwd);
    }
}

/// Remove any dots from the path by popping components as needed.
fn dedot<P: AsRef<Path>>(path: P) -> PathBuf {
    #[allow(clippy::enum_glob_use)]
    use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};

    let mut comps = vec![];
    for component in path.as_ref().components() {
        match component {
            p @ Prefix(_) => comps = vec![p],
            r @ RootDir if comps.iter().all(|c| matches!(c, Prefix(_))) => comps.push(r),
            r @ RootDir => comps = vec![r],
            CurDir => {}
            ParentDir if comps.iter().all(|c| matches!(c, Prefix(_) | RootDir)) => {}
            ParentDir => {
                let _ = comps.pop();
            }
            c @ Normal(_) => comps.push(c),
        }
    }

    comps.iter().map(|c| c.as_os_str()).collect()
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::{dedot, LitterTray};
    use std::{fs, path::PathBuf};

    #[test]
    fn drop_removes_tempdir() {
        let mut path = PathBuf::new();
        LitterTray::try_with(|tray| {
            let _ = tray.create_text("test.txt", "Hello, world!").unwrap();
            path = tray.directory().to_path_buf();
            assert!(fs::exists(&path)?);
            assert!(fs::exists("test.txt")?);
            Ok(())
        })
        .unwrap();
        assert!(!fs::exists(path).unwrap());
    }

    #[test]
    fn return_value() {
        assert_eq!(LitterTray::try_with(|_tray| { Ok(42) }).unwrap(), 42);
    }

    fn getcwd() -> Option<PathBuf> {
        std::env::current_dir().ok()
    }

    #[test]
    fn working_directory_restored() {
        let prev_dir = getcwd();
        let mut tray_dir = PathBuf::new();
        LitterTray::run(|tray| {
            tray_dir = tray.directory().to_path_buf();
            assert_ne!(prev_dir.unwrap_or_default(), tray_dir);
        });
        // We can't usefully assert that prev_dir == getcwd(), because another test might be running
        // (now the tray has been dropped and the global lock released). However we can meaningfully
        // assert that we are no longer in the tray dir, and that it has been removed.
        assert_ne!(tray_dir, getcwd().unwrap_or_default());
        assert!(!std::fs::exists(tray_dir).unwrap());
    }

    #[test]
    fn absolute_path() {
        LitterTray::try_with(|tray| {
            // Creating a file by absolute path within the sandbox works
            let mut path = PathBuf::from(tray.directory());
            path.push("file.txt");
            let _ = tray.create_text(path, "hi").unwrap();
            Ok(())
        })
        .unwrap();
    }

    fn outside_path() -> &'static str {
        if cfg!(windows) {
            "C:\\not-a-litter-tray"
        } else {
            "/not-a-litter-tray"
        }
    }

    #[test]
    fn absolute_path_outside_fails_our_error_returned() {
        let inner_error = LitterTray::try_with(|tray| {
            // Creating a file by absolute path outside the sandbox is blocked
            let mut path = PathBuf::new();
            path.push(outside_path());
            let res = tray.create_text(path, "hi").unwrap_err();
            Ok(res)
        })
        .unwrap();
        let crate::Error::Uncontained(_) = inner_error else {
            panic!("Wrong inner error type; got {inner_error:?}");
        };
    }

    #[test]
    fn inner_error_coerced_to_anyhow() {
        let e = LitterTray::try_with(|tray| {
            // Creating a file by absolute path outside the sandbox is blocked
            let mut path = PathBuf::new();
            path.push(outside_path());
            let res = tray.create_text(path, "hi")?;
            Ok(res)
        })
        .unwrap_err();
        assert!(e
            .to_string()
            .contains("requested path is outside of the sandbox"));
        // but this has been coerced to anyhow, so we cannot match it against crate::Error.
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_work() {
        LitterTray::try_with(|tray| {
            let _ = tray.make_symlink("file1", "file2")?;
            // The symlink does not exist, because the original file is not there
            assert!(!std::fs::exists("file2")?);
            // Now create file1 and confirm that file2 exists
            let _ = tray.create_text("file1", "hi there");
            assert!(std::fs::exists("file2")?);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_returning_anyhow_result() -> anyhow::Result<()> {
        LitterTray::try_with(|_| Ok(()))
    }

    #[test]
    fn dedot_test() {
        assert_eq!(dedot(PathBuf::from("/./a/../b/c/.")), PathBuf::from("/b/c"));
        assert_eq!(dedot(PathBuf::from(".")), PathBuf::from(""));
    }

    #[test]
    #[ignore = "broken"]
    fn panic_in_closure_propagates() {
        // CAUTION: When run in sync mode, this test poisons the global mutex. Later tests will fail!
        // rusty_fork incurs the race condition described above.
        // Omit this test from CI, until we can figure out a proper solution.
        let r = std::panic::catch_unwind(|| {
            LitterTray::run(|_| {
                panic!("at the disco");
            });
        });
        assert!(r.is_err());
    }
}

/*
 * Ugh, this is a bit curly...
 *
 * Simply disabling this module with #[cfg(all(test, feature = "async", async_supported))]
 * isn't sufficient; the 1.81 compiler still complains that async closures are unstable.
 *
 * However, cfg_if actually removes the tokens from the AST altogether. So we'll use that.
 */
cfg_if::cfg_if! { if #[cfg(async_supported)] {
#[cfg(all(test, feature = "async", async_supported))]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test_async {
    #[allow(unused_imports)]
    use crate::LitterTray;

    #[cfg(feature = "async")]
    #[test]
    fn async_closure() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            LitterTray::try_with_async(async |tray| {
                let _ = tray.create_text("test.txt", "Hello, world!").unwrap();
                assert_eq!(
                    tokio::fs::read_to_string("test.txt").await.unwrap(),
                    "Hello, world!"
                );
                Ok(())
            })
            .await
            .unwrap();
        });
    }

    #[tokio::test]
    async fn async_test() {
        LitterTray::try_with_async(async |tray| {
            let _ = tray.create_text("test.txt", "Hello, world!").unwrap();
            assert_eq!(
                tokio::fs::read_to_string("test.txt").await.unwrap(),
                "Hello, world!"
            );
            Ok(())
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn async_test_returning_anyhow_result() -> anyhow::Result<()> {
        LitterTray::try_with_async(async |_| Ok(())).await
    }

    #[tokio::test]
    #[should_panic = "at the disco"]
    async fn panic_in_async_propagates() {
        let _ = LitterTray::try_with_async(async |_| {
            panic!("at the disco");
            #[allow(unreachable_code)] // sets the return type of the closure
            Ok(())
        })
        .await;
    }
} // mod test_async
}} // cfg_if!
