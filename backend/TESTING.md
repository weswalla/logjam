# Local Testing Guide for Rust Backend

This guide explains how to run the same tests locally that run in the GitHub Actions workflow, ensuring you catch compilation and test errors before creating a PR.

## Prerequisites

### 1. Install Rust

If Rust is not already installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

After installation, activate Rust in your current shell:

```bash
source "$HOME/.cargo/env"
```

To make Rust available in all future shells, add this line to your `~/.bashrc` or `~/.zshrc`:

```bash
source "$HOME/.cargo/env"
```

### 2. Verify Installation

```bash
rustc --version
cargo --version
```

You should see output like:
```
rustc 1.90.0 (1159e78c4 2025-09-14)
cargo 1.90.0 (840b83a10 2025-07-30)
```

## Running Tests Locally

### Quick Test Before PR

Navigate to the backend directory and run these two commands:

```bash
cd backend
cargo build
cargo test
```

This will:
1. **`cargo build`** - Compile all code and catch compilation errors
2. **`cargo test`** - Run all unit tests, integration tests, and doctests

### What the GitHub Workflow Runs

The GitHub Actions workflow (`.github/workflows/rust-tests.yml`) runs:

```bash
cargo nextest run
```

`cargo-nextest` is a faster test runner, but `cargo test` runs the same tests and is simpler to use locally.

### Optional: Install cargo-nextest

If you want to match the CI environment exactly:

```bash
cargo install cargo-nextest --locked
```

Then run tests with:

```bash
cargo nextest run
```

## Understanding Test Output

### Successful Build

```
Compiling backend v0.1.0 (/path/to/backend)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.38s
```

### Successful Tests

```
test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Common Issues

#### Compilation Errors

If you see errors like `error[E0308]: mismatched types`, fix them before committing:

```
error[E0308]: mismatched types
  --> src/application/services/sync_service.rs:192:60
   |
192 |                     cb(SyncEvent::FileDeleted { file_path: path });
   |                                                            ^^^^ expected `PathBuf`, found `&PathBuf`
```

#### Test Failures

Test failures show which assertion failed:

```
---- infrastructure::parsers::logseq_markdown::tests::test_extract_page_references stdout ----
thread 'test_extract_page_references' panicked at src/infrastructure/parsers/logseq_markdown.rs:287:9:
assertion `left == right` failed
  left: "another page"
 right: "tag"
```

## Test Organization

The project has several test types:

1. **Unit tests** (`#[test]` in module files)
   - Test individual functions and methods
   - Found at the bottom of source files in `#[cfg(test)]` modules

2. **Integration tests** (`tests/` directory)
   - Test multiple components working together
   - Located in `backend/tests/integration_test.rs` and `backend/tests/application_integration_test.rs`

3. **Documentation tests** (in doc comments)
   - Example code in `///` doc comments

## Running Specific Tests

```bash
# Run all tests
cargo test

# Run tests in a specific file
cargo test --test integration_test

# Run tests matching a pattern
cargo test page_reference

# Run a specific test
cargo test test_extract_page_references

# Show test output (println! statements)
cargo test -- --nocapture

# Run tests with detailed output
cargo test -- --show-output
```

## Checking for Warnings

```bash
# Build with all warnings
cargo build

# Build in release mode (optimized)
cargo build --release

# Check without building
cargo check
```

## Continuous Development Workflow

Recommended workflow when making changes:

```bash
# 1. Make your changes to source files

# 2. Check compilation frequently (fast)
cargo check

# 3. Run tests (slower but thorough)
cargo test

# 4. Before committing, run both
cargo build && cargo test

# 5. Commit only if both pass
git add .
git commit -m "your message"
```

## Troubleshooting

### Rust environment not found

```bash
# Ensure cargo is in PATH
source "$HOME/.cargo/env"

# Or use absolute path
/home/cyrus/.cargo/bin/cargo test
```

### Tests pass locally but fail in CI

1. Check you're testing the same code (committed all changes)
2. Check for differences in dependencies (try `cargo update`)
3. Look at the CI logs for specific error messages

### Dependency issues

```bash
# Update dependencies
cargo update

# Clean build artifacts
cargo clean

# Rebuild from scratch
cargo clean && cargo build
```

## Summary

**Before every PR, run:**

```bash
cd backend
cargo build && cargo test
```

If both commands succeed with no errors, your code will pass the GitHub Actions workflow checks.
