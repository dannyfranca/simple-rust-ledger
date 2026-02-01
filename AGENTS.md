# Guidelines

- correctness: all cases must be heavily tested, including edge cases
- safety and robustnaess: strong error handling, careful consideration of dangerouns choices
- efficient usage of system resources
- maintainability: clean code is a priority over efficient code as humans will be reading and reviewing the code.

## Valiating code changes

- cargo build --release
- cargo clippy -- -D warnings
- cargo fmt --check
- cargo test
