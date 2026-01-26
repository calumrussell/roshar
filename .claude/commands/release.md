# Release Packages

Run cargo-release for the specified packages.

## Arguments

- `$ARGUMENTS` - Space-separated list of package names to release (e.g., `roshar-types roshar-clients`)

## Available Packages

The following packages are available in this workspace:
- roshar-ws
- roshar-ws-mgr
- roshar-bt
- roshar-types
- roshar-clients

## Instructions

1. Parse the `$ARGUMENTS` to get the list of packages to release
2. For each package specified, run `cargo release` with the appropriate flags
3. If no packages are specified, ask the user which packages they want to release

## Execution

Run the following command for each package:

```bash
cargo release --package <package-name> --execute
```

If the user wants a dry run first (no `--execute` flag), ask them before running.

By default, perform a dry run first to show what will happen, then ask for confirmation before executing the actual release.
