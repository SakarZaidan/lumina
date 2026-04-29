# Contributing to Lumina

Thank you for your interest in contributing to Lumina! Whether it's bug fixes, feature implementations, or documentation improvements, we welcome your help.

## Development Workflow

1. **Fork the Repository**: Clone your fork to your local machine.
2. **Setup**: Ensure you have the latest stable [Rust](https://rustup.rs/) and `ffmpeg` installed.
3. **Branching**: Create a new branch for your feature or bug fix:
   ```bash
   git checkout -b feature/my-feature-name
   ```
4. **Implementation**: Ensure your code follows the existing style and architectural patterns.
5. **Testing**: Add or update unit/integration tests within the relevant crate.
6. **Verification**: Run the full test suite before submitting:
   ```bash
   cargo test --workspace
   ```

## Pull Request Guidelines

- **Focus**: Keep pull requests focused on a single feature or bug fix.
- **Documentation**: If your change impacts the LSF schema or adds a new object type, update the `README.md` and the appropriate `lumina-schema` crate definitions.
- **Benchmarks**: For performance-critical changes, please include results from the stress tests found in `lumina-core`.

## Code Style

- **Formatting**: We use `cargo fmt` as the source of truth for code style. Please run it before committing.
- **Safety**: Lumina avoids `unsafe` code whenever possible. If your contribution requires `unsafe` blocks, please provide a thorough justification and include the necessary safety comments.

## Reporting Issues

If you find a bug, please open an issue with:
- A clear description of the problem.
- A minimal LSF scene file that reproduces the issue.
- Your environment details (OS, GPU, Rust version).
