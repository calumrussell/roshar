## Version Bumping

Before creating the PR, bump the patch version in `Cargo.toml` for each crate that was modified.

For example, if `crates/foo/src/lib.rs` was changed, update `crates/foo/Cargo.toml`:
```toml
version = "0.1.0"  # bump to "0.1.1"
```
