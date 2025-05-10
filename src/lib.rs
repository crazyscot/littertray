// Copyright (c) 2020 Sergio Benitez, (c) 2025 Ross Younger
//! Lightweight sandboxing for tests that write to the filesystem
//!
//!
//! This is a derivative work of
//! [`figment::Jail`](https://docs.rs/figment/latest/figment/struct.Jail.html)
//! but simpler (no environment variables), and it supports async closures.

use std::fs::{self, File};
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::TempDir;
use thiserror::Error;

/// The result type used by [`LitterTray`]
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The error type used by [`LitterTray`]
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
/// <div class="warning">
/// While this crate contains no <i>unsafe</i> Rust, it is not without limitation.
/// <tt>LitterTray</tt> uses a global lock to prevent tests from conflicting when run in parallel
/// (which is cargo's default behaviour).
/// This has the effect of serialising your tests.
/// If you want to parallelise testing, consider
/// <a href="https://docs.rs/rusty-fork/latest/rusty_fork/">rusty_fork</a>.
/// </div>
///
#[derive(Debug)]
pub struct LitterTray {
    canonical_dir: PathBuf,
    _dir: TempDir,
    saved_cwd: PathBuf,
}

/// This mutex ensures that only one test can use a litter tray at once.
/// This is necessary because it changes the process working directory.
/// If you want to parallelise testing, consider [`rusty_fork`](https://docs.rs/rusty-fork/latest/rusty_fork/).
static G_LOCK: Mutex<()> = Mutex::new(());

impl LitterTray {
    /// Runs a closure in a new sandbox, passing the sandbox to the closure.
    ///
    /// # Returns
    /// Whatever the closure returns.
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure (see [Mutex#poisoning](Mutex#poisoning))
    ///
    /// # Example
    ///
    /// ```
    /// use littertray::LitterTray;
    ///
    /// let result = LitterTray::try_with(|tray| {
    ///   let _ = tray.create_text("test.txt", "Hello, world!")?;
    ///   assert_eq!(std::fs::read_to_string("test.txt")?, "Hello, world!");
    ///   Ok(42)
    /// }).unwrap();
    /// ```
    pub fn try_with<R, F: FnOnce(&mut LitterTray) -> Result<R>>(f: F) -> Result<R> {
        let _guard = G_LOCK.lock().unwrap();
        let dir = TempDir::new()?;
        let mut tray = LitterTray {
            canonical_dir: dir.path().canonicalize()?,
            _dir: dir,
            saved_cwd: std::env::current_dir()?,
        };
        std::env::set_current_dir(tray.directory())?;
        let outcome = f(&mut tray);
        drop(tray); // Force cleanup & reset of working directory before we release the lock
        outcome
    }

    /// Runs a closure in a sandbox, passing the sandbox to the closure.
    ///
    /// This is a convenience wrapper for [`LitterTray::try_with`] which returns nothing.
    /// The closure is expected to return nothing.
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure (see [Mutex#poisoning](Mutex#poisoning))
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

    /// Runs an async closure in a new sandbox, passing the sandbox to the closure.
    ///
    /// # Returns
    /// Whatever the closure returns.
    ///
    /// # Panics
    ///
    /// If the global lock was poisoned by a panic in a previous closure (see [Mutex#poisoning](Mutex#poisoning))
    #[cfg(feature = "async")]
    pub async fn try_with_async<R, F: AsyncFnOnce(&mut LitterTray) -> Result<R>>(
        f: F,
    ) -> Result<R> {
        let _guard = G_LOCK.lock().unwrap();
        let dir = TempDir::new()?;
        let mut tray = LitterTray {
            canonical_dir: dir.path().canonicalize()?,
            _dir: dir,
            saved_cwd: std::env::current_dir()?,
        };
        std::env::set_current_dir(tray.directory())?;
        let outcome = f(&mut tray).await;
        drop(tray); // Force cleanup & reset of working directory before we release the lock
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
    use std::path::Component::*;

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
mod test {
    use super::{dedot, LitterTray};
    use rusty_fork::rusty_fork_test;
    use std::{fs, path::PathBuf};

    fn getcwd() -> PathBuf {
        std::env::current_dir().unwrap()
    }

    // These tests run in forks to enable parallelisation.
    // They change the working directory and would trample each other;
    // while the global lock prevents trouble, running them this way
    // allows for parallelisation.

    rusty_fork_test! {
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

        #[test]
        fn working_directory_restored() {
            let prev_dir = getcwd();
            LitterTray::run(|tray| {
                let _ = tray.create_text("hi", "hi").unwrap();
                assert_ne!(prev_dir, getcwd());
            });
            assert_eq!(prev_dir, getcwd());
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

        #[test]
        fn absolute_path_outside_fails() {
            LitterTray::try_with(|tray| {
                // Creating a file by absolute path outside the sandbox is blocked
                let mut path = PathBuf::new();
                path.push("/not-a-litter-tray");
                let _ = tray.create_text(path, "hi").unwrap_err();
                Ok(())
            })
            .unwrap();
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
    }

    #[cfg(feature = "async")]
    rusty_fork_test! {
        // trap: cfg macros don't work within rusty_fork_test, you have to put them outside the macro.
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
    }

    #[test]
    fn dedot_test() {
        assert_eq!(dedot(PathBuf::from("/./a/../b/c/.")), PathBuf::from("/b/c"));
        assert_eq!(dedot(PathBuf::from(".")), PathBuf::from(""));
    }
}
