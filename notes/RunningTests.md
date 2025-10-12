# Running Tests

You can run all Rust tests with:

```bash
cargo test
```

This will run:
- Unit tests (in `src/` files with `#[test]` functions)
- Integration tests (in `tests/` directory)
- Doc tests (examples in documentation comments)

Other useful test commands:

```bash
# Run tests with output from println! statements
cargo test -- --nocapture

# Run tests in parallel (default) or single-threaded
cargo test -- --test-threads=1

# Run only tests matching a pattern
cargo test test_page_reference

# Run only integration tests
cargo test --test integration_test

# Run only unit tests (exclude integration tests)
cargo test --lib

# Run with verbose output
cargo test --verbose
```

Since your project structure has both unit tests (in the domain modules) and integration tests (in `backend/tests/`), `cargo test` will run all of them.
