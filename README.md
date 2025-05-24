[![Crates.io](https://img.shields.io/crates/v/littertray.svg)](https://crates.io/crates/littertray)
![GitHub code size in bytes](https://img.shields.io/github/languages/code-size/crazyscot/littertray)
[![Build status](https://github.com/crazyscot/littertray/actions/workflows/ci.yml/badge.svg)](https://github.com/crazyscot/littertray/actions/workflows/ci.yml)
[![Documentation](https://img.shields.io/docsrs/engineering-repr)](https://docs.rs/littertray/)
![GitHub License](https://img.shields.io/github/license/crazyscot/littertray)
[![codecov](https://codecov.io/gh/crazyscot/littertray/graph/badge.svg?token=EA3A7ETRNZ)](https://codecov.io/gh/crazyscot/littertray)

Lightweight sandboxing for tests that write to the filesystem

## Description

This is little more than a convenience wrapper to
[`tempdir::TempDir`](https://docs.rs/tempdir/latest/tempdir/struct.TempDir.html).
You provide a closure that runs in the sandbox; there are convenience methods to create files, directories and so forth.

**Crucially, the crate changes working directory into the sandbox while it is active.** This allows the unit under test to read and write files into the sandbox, provided they are expressed as relative paths.

When the struct is dropped, the tempdir is cleaned up.

### See also

This crate is inspired by and is a derivative work of [`figment::Jail`](https://docs.rs/figment/latest/figment/struct.Jail.html).

Differences:

- This crate adds support for async closures.
- This crate does not currently support environment variables.

## Examples

```rust
use littertray::LitterTray;

let result = LitterTray::try_with(|tray| {
  let _ = tray.create_text("test.txt", "Hello, world!")?;
  assert_eq!(std::fs::read_to_string("test.txt")?, "Hello, world!");
  Ok(())
}).unwrap();
```

This is how you'd do something similar with an async closure:

```rust
use littertray::LitterTray;

let result = LitterTray::try_with_async(async |tray| {
    let _ = tray.create_text("test.txt", "Hello, world!")?;
    assert_eq!(
        tokio::fs::read_to_string("test.txt").await.unwrap(),
        "Hello, world!"
    );
    Ok(())
})
.await
.unwrap();
```

## Limitations

- The sandboxing is trivial to escape, by using absolute paths. This is a deliberate design decision.
- The sandboxing relies on changing the process's working directory.
  It uses a global (per-process) lock which prevents tests from conflicting,
  but it has no way of knowing about other changes to the working directory.
- The lock has the effect of forcing tests to run in serial. To allow parallelisation, consider
  using [`rusty-fork`](https://crates.io/crates/rusty-fork) or similar.
