# Contributing to Browsion

Thank you for your interest in contributing to Browsion! 🎉

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/browsion.git`
3. Create a branch: `git checkout -b feature/your-feature-name`
4. Make your changes
5. Test your changes
6. Commit: `git commit -m "Add some feature"`
7. Push: `git push origin feature/your-feature-name`
8. Open a Pull Request

## Development Setup

### Prerequisites

- Node.js (v18+)
- Rust (latest stable)
- Platform-specific dependencies:
  - **Linux**: `libwebkit2gtk-4.1-dev`, `xdotool`, `wmctrl`
  - **Windows**: WebView2
  - **macOS**: Xcode Command Line Tools

### Running Locally

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Project Structure

```
browsion/
├── src/                # React frontend
│   ├── components/     # UI components
│   ├── api/           # Tauri API wrappers
│   └── types/         # TypeScript types
├── src-tauri/         # Rust backend
│   ├── src/
│   │   ├── config/    # Configuration management
│   │   ├── process/   # Process management
│   │   ├── window/    # Window activation
│   │   └── tray/      # System tray
│   └── capabilities/  # Tauri permissions
└── docs/              # Documentation
```

## Code Style

### Rust

- Follow Rust standard style (`cargo fmt`)
- Run `cargo clippy` before committing
- Add tests for new functionality

### TypeScript/React

- Use functional components with hooks
- Follow existing naming conventions
- Use TypeScript types, avoid `any`
- Format with Prettier

## Testing

```bash
# Rust tests
cd src-tauri && cargo test

# Frontend build test
npm run build

# Manual testing
npm run tauri dev
```

## Commit Messages

Follow the conventional commits format:

```
type(scope): subject

body

footer
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
- `feat(clone): add profile clone functionality`
- `fix(activation): fix window activation on Linux`
- `docs(readme): update installation instructions`

## Pull Request Guidelines

1. **Description**: Clearly describe what your PR does
2. **Testing**: Test on at least one platform
3. **Documentation**: Update docs if needed
4. **Screenshots**: Include for UI changes
5. **Breaking Changes**: Clearly mark and explain

## Feature Requests

Open an issue with:
- Clear description of the feature
- Use cases
- Expected behavior
- Alternative solutions considered

## Bug Reports

Include:
- Operating system and version
- Steps to reproduce
- Expected vs actual behavior
- Screenshots if applicable
- Logs/error messages

## Questions?

- Open a Discussion for general questions
- Open an Issue for bugs
- Check existing documentation first

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing! 🚀
