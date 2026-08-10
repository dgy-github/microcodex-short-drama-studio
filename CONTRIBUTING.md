# Contributing to MicrocodeX Short Drama Studio

Thank you for considering contributing to this project! 🎉

---

## ⚠️ Alpha Status

This is an **alpha project** with known limitations:
- P1: Evaluation system not validated (seeded_defect_detection = 0.0)
- P7: No professional screenwriter review
- P10: Clean VM acceptance test not executed

All generated content is **advisory/non-promotable**. See [README.md](README.md) for details.

---

## 🤝 How to Contribute

### High-Value Contributions

1. **Execute P10 Acceptance Test**
   - Run `docs/CLEAN_VM_ACCEPTANCE_TEST.md` on clean Windows VM
   - Document results and any issues found
   - This is a **critical blocker** for production readiness

2. **Improve Judge Stability**
   - Analyze why `seeded_defect_detection = 0.0`
   - Propose improved judge prompts
   - Test alternative judge models/families

3. **Add Desktop Tests**
   - Current coverage: 19 tests for ~4,067 lines
   - Target: Integration tests for key workflows
   - See `apps/desktop/src-tauri/` for test structure

4. **Documentation Improvements**
   - Add examples and tutorials
   - Translate to other languages
   - Improve troubleshooting guides

5. **Bug Reports**
   - File detailed issues with reproduction steps
   - Include logs and system information
   - Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md) first

---

## 🚀 Getting Started

### Prerequisites

- **Rust**: 1.88.0 (see `rust-toolchain.toml`)
- **Node.js**: 22.14.0 (see `.nvmrc`)
- **Python**: 3.12.10 (see `.python-version`)
- **Windows 10+** with Windows Credential Manager

### Setup Development Environment

**Automated** (recommended):
```powershell
python scripts/setup_dev_environment.py
```

**Manual**:
```powershell
# Clone repository
git clone https://github.com/dgy-github/microcodex-short-drama-studio.git
cd microcodex-short-drama-studio

# Set up Python environment
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -e sidecar

# Initialize project
python scripts/init_project.py --check
python scripts/init_project.py --name "MicrocodeX Short Drama Studio"

# Verify installation
cargo test --workspace --all-features
python -m unittest discover -s sidecar -p "test_*.py"
```

See [README.md](README.md#quick-start) for detailed setup instructions.

---

## 📋 Development Workflow

### Before You Start

1. Read [HANDOFF.md](HANDOFF.md) - current project status
2. Read [docs/ROADMAP.md](docs/ROADMAP.md) - phase sequencing
3. Read [AGENTS.md](AGENTS.md) - development rules
4. Query `docs/project-memory/PROJECT_REGISTRY.yaml` - project structure

### Making Changes

1. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow existing code style
   - Add tests for new functionality
   - Update documentation as needed

3. **Run quality checks**
   ```powershell
   # Format code
   cargo fmt --all
   
   # Run linter
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   
   # Run tests
   cargo test --workspace --all-features
   cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
   python -m unittest discover -s sidecar -p "test_*.py"
   python -m unittest discover -s eval/tools -p "test_*.py"
   
   # Project integrity
   python scripts/init_project.py --check
   ```

4. **Commit your changes**
   ```bash
   git add .
   git commit -m "Brief description of changes
   
   - Detailed point 1
   - Detailed point 2
   
   Co-Authored-By: Your Name <your.email@example.com>"
   ```

5. **Push and create PR**
   ```bash
   git push origin feature/your-feature-name
   ```
   Then create a Pull Request on GitHub.

---

## 🎨 Code Standards

### Rust

- **Function target**: 40 logical lines, extract at 60, explain if >80
- **File target**: 400 lines, split at 500, document exception if >700
- **No unsafe code** in production (core crates)
- **Error handling**: Use `Result<T, Error>` with `thiserror`
- **Comments**: Explain security boundaries, invariants, concurrency
- **Documentation**: Public APIs must have doc comments

Example:
```rust
/// Validates candidate decision trace before permanent storage.
///
/// # Errors
/// Returns `CandidateTraceError` if:
/// - Identity fields are blank
/// - Less than 2 candidates
/// - Multiple selected candidates
pub fn validate(&self) -> Result<(), CandidateTraceError> {
    // Implementation
}
```

### Python

- **Function target**: 40 lines
- **File target**: 400 lines
- **Type hints** for public functions
- **Docstrings** for modules and functions
- **Follow PEP 8**

### Svelte

- **Component target**: <300 lines
- **Nesting**: <3 levels
- **Reactivity**: Clear and explicit

---

## 🧪 Testing Guidelines

### Test Requirements

- **New features**: Must include tests
- **Bug fixes**: Must include regression test
- **Breaking changes**: Must update existing tests

### Test Structure

**Rust**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_describes_behavior() {
        // Arrange
        let input = create_test_data();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

**Python**:
```python
import unittest

class TestFeature(unittest.TestCase):
    def test_name_describes_behavior(self):
        # Arrange
        input_data = create_test_data()
        
        # Act
        result = function_under_test(input_data)
        
        # Assert
        self.assertEqual(result, expected)
```

---

## 📝 Commit Message Guidelines

### Format

```
<type>: <subject>

<body>

<footer>
```

### Types

- **feat**: New feature
- **fix**: Bug fix
- **docs**: Documentation changes
- **style**: Code style (formatting, no logic change)
- **refactor**: Code refactoring
- **test**: Add or update tests
- **chore**: Maintenance tasks

### Example

```
feat: add genre pack validation script

- Validate genre-pack JSON against schema
- Check for duplicate IDs
- Verify file references exist
- Add unit tests for validation logic

Resolves #123

Co-Authored-By: Contributor Name <email@example.com>
```

---

## 🐛 Bug Reports

### Before Filing

1. Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md)
2. Search [existing issues](https://github.com/dgy-github/microcodex-short-drama-studio/issues)
3. Try to reproduce on clean installation

### Bug Report Template

Include:
- Clear description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Environment (OS, Rust/Python/Node versions)
- Relevant logs
- Screenshots if applicable

---

## ✨ Feature Requests

Feature requests are welcome! Please:

1. **Check roadmap** first: [docs/ROADMAP.md](docs/ROADMAP.md)
2. **Explain use case**: Why is this needed?
3. **Describe alternatives**: What did you consider?
4. **Consider scope**: Does this fit the project's goals?

**Note**: P11-P18 features are already planned but not scheduled.

---

## 📖 Documentation Contributions

Documentation improvements are always welcome:

- Fix typos and grammar
- Add examples and tutorials
- Improve clarity
- Translate to other languages
- Add diagrams and screenshots

---

## 🔒 Security Issues

**Do not file public issues for security vulnerabilities.**

Instead, email the maintainer directly. See [SECURITY.md](SECURITY.md) for full policy.

---

## 🤔 Questions and Discussions

For questions or general discussion:
- **GitHub Discussions**: Preferred for Q&A
- **Issues**: For concrete bugs or feature requests only

---

## 📜 Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inclusive environment for all contributors.

### Our Standards

**Expected behavior**:
- Be respectful and inclusive
- Welcome newcomers
- Accept constructive criticism
- Focus on what's best for the project

**Unacceptable behavior**:
- Harassment or discrimination
- Trolling or insulting comments
- Personal or political attacks
- Publishing others' private information

### Enforcement

Report instances of unacceptable behavior to the project maintainer.

---

## 📄 License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

## 🙏 Thank You!

Thank you for contributing! Your efforts help make this project better.

**Questions?** Ask in [GitHub Discussions](https://github.com/dgy-github/microcodex-short-drama-studio/discussions) or file an issue.
