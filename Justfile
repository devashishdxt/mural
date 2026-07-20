# List available commands
_default:
    just --list

# Format Rust code
fmt:
    cargo fmt --all

# Run unit and doc tests
test:
    cargo test --workspace

# Run Clippy with warnings denied
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Build crate documentation with rustdoc warnings denied
doc:
    RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps

# Generate LCOV coverage data for tools such as cargo-crap (requires nightly)
coverage-lcov:
    mkdir -p target/llvm-cov
    cargo +nightly llvm-cov --workspace --lcov --output-path target/llvm-cov/lcov.info

# Report coverage summary excluding inline test modules (requires nightly)
coverage: coverage-lcov
    cargo +nightly llvm-cov report --summary-only

# Generate an HTML coverage report excluding inline test modules (requires nightly)
coverage-html:
    cargo +nightly llvm-cov --workspace --html

# Open the HTML coverage report
coverage-report:
    open target/llvm-cov/html/index.html

# Run the full local validation suite
check: fmt test clippy doc

# Report CRAP complexity using nightly LCOV coverage
crap: coverage-lcov
    cargo crap --lcov target/llvm-cov/lcov.info

# Fail if any function exceeds the configured CRAP threshold
crap-ci: coverage-lcov
    cargo crap --lcov target/llvm-cov/lcov.info --fail-above --summary

# Run validation plus coverage and CRAP gates
ci: check coverage crap-ci
