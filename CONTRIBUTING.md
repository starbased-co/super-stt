# Contributing to Super STT

Thank you for your interest in contributing to Super STT! This document provides guidelines for contributing to the project.

## Development Setup

1. **Prerequisites**:
   - Rust 1.70+ (edition 2024)
   - Just task runner: `cargo install just`
   - CUDA (optional, for GPU acceleration)

2. **Clone and Build**:
   ```bash
   git clone https://github.com/jorge-menjivar/super-stt.git
   cd super-stt
   
   # Install development version
   just install-daemon
   just install-app
   just install-applet  # Optional
   ```

3. **Development Commands**:
   ```bash
   # Run daemon in development mode
   just run-daemon
   
   # Run desktop app
   just run-app
   
   # Run COSMIC applet
   just run-applet
   ```

## Code Style and Standards

- **Rust**: Follow standard Rust conventions and use `cargo fmt`
- **Security**: All external inputs must be validated using the shared validation framework
- **Testing**: Add tests for new functionality, especially security-critical code
- **Documentation**: Document public APIs and security-relevant functions

## Security Guidelines

- Never bypass the process authentication system
- All network communication must validate inputs
- Use the shared validation framework in `super-stt-shared/src/validation/`
- Follow the development vs production security model (debug vs release builds)
- Run security audits before proposing changes: `cargo audit`

## Pull Request Process

1. **Before submitting**:
   - Run `cargo test` to ensure all tests pass
   - Run `cargo fmt` to format code
   - Run `cargo clippy` to check for warnings
   - Run `cargo audit` to check for security vulnerabilities
   - Test on both debug and release builds

2. **Pull Request Requirements**:
   - Clear description of changes
   - Reference any related issues
   - Include tests for new functionality
   - Update documentation if needed

3. **Review Process**:
   - All PRs require review
   - Security-related changes require additional scrutiny
   - CI must pass before merging

## Reporting Security Issues

If you discover a security vulnerability, please:

1. **Do not** open a public issue
2. Email security concerns to: jorge@menjivar.ai
3. Include detailed reproduction steps
4. Allow reasonable time for response before public disclosure

## Code of Conduct

- Be respectful and inclusive
- Focus on constructive feedback
- Help maintain a welcoming environment for all contributors

## License

By contributing to Super STT, you agree that your contributions will be licensed under the GPL-3.0-only license.

## Questions?

- Open an issue for feature requests or bugs
- Join discussions in existing issues
- Contact: jorge@menjivar.ai