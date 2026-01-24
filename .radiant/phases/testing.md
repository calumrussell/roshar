## Testing Commands

Run the following commands to verify the implementation:

1. Check for compilation errors:
   ```bash
   cargo check --all-targets
   ```

2. Format code:
   ```bash
   cargo fmt --all
   ```

3. Run lints:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

4. Run tests:
   ```bash
   cargo test --all
   ```

All commands must pass before moving to the review phase.
