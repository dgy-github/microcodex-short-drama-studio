## 📋 Description

<!-- Briefly describe what this PR does -->

## 🎯 Related Issue

Closes #<!-- issue number -->

## 🔄 Type of Change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Code refactoring
- [ ] Test improvement
- [ ] Chore (maintenance, dependencies, etc.)

## 🧪 Testing

### What has been tested?

- [ ] All existing tests pass
- [ ] New tests added for this change
- [ ] Tested manually on Windows

### Test Commands

```powershell
# Commands used to test
cargo test --workspace --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
python -m unittest discover -s sidecar -p "test_*.py"
python -m unittest discover -s eval/tools -p "test_*.py"
```

## 📝 Checklist

### Code Quality

- [ ] Code follows the project's style guidelines
- [ ] Self-review of code performed
- [ ] Comments added for complex code
- [ ] No new warnings from clippy
- [ ] Code formatted with `cargo fmt --all`

### Documentation

- [ ] Documentation updated (if needed)
- [ ] CHANGELOG.md updated (if user-facing change)
- [ ] README.md updated (if needed)

### Testing

- [ ] New tests added (if applicable)
- [ ] All tests pass locally
- [ ] Edge cases considered

## 📸 Screenshots

<!-- If applicable, add screenshots to demonstrate the changes -->

## 🔍 Additional Notes

<!-- Any additional information that reviewers should know -->

## 📚 References

<!-- Links to related issues, docs, or external resources -->
