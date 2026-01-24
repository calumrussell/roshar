## Project Structure

This is a Rust workspace with multiple crates.

## Version Bumping

When making changes to a crate, bump the patch version in that crate's `Cargo.toml`.

For example, if you modify code in `crates/foo/`, update `crates/foo/Cargo.toml`:
```toml
version = "0.1.0"  # bump to "0.1.1"
```
