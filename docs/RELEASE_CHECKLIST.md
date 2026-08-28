# Release Checklist

**Purpose**: Ensure all requirements are met before releasing a new version.

**Version**: For `0.1.0-alpha.1` → `0.1.0` (first stable release)

---

## Pre-Release Requirements

### ✅ Code Quality

- [ ] All tests pass
  ```powershell
  cargo test --workspace --all-features
  cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
  python -m unittest discover -s sidecar -p "test_*.py"
  python -m unittest discover -s eval/tools -p "test_*.py"
  ```

- [ ] No clippy warnings
  ```powershell
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  ```

- [ ] Code formatted
  ```powershell
  cargo fmt --all -- --check
  ```

- [ ] Project integrity check passes
  ```powershell
  python scripts/init_project.py --check
  ```

- [ ] No TODO/FIXME/XXX in production code
  ```powershell
  grep -r "TODO\|FIXME\|XXX" crates/*/src/*.rs
  # Should return empty
  ```

### ✅ P10 Exit Conditions

- [ ] **Clean VM acceptance test passed** (CRITICAL)
  - See: `docs/CLEAN_VM_ACCEPTANCE_TEST.md`
  - Document results in: `docs/acceptance-test-results/YYYYMMDD-clean-vm-test.md`

- [ ] Install → Configure → Story → Export → Upgrade → Rollback
  - All steps completed successfully
  - No manual workarounds required

- [ ] Provider soak test passed
  - DeepSeek: X iterations successful
  - Bailian: X iterations successful
  - Document in: `docs/provider-soak-results.md`

### ✅ Evaluation & Quality (P1/P7 Blockers)

⚠️ **CRITICAL BLOCKERS** - Cannot claim stable release without these:

- [ ] **P1: Judge calibration complete**
  - [ ] `seeded_defect_detection` ≥ 0.75 (currently 0.0 over `pairs_total = 1`;
        a single narrow pair, so only 0.0 or 1.0 is attainable and the number is
        not an estimate — the adversarial set has to grow before this gate means
        anything. The broad `motive-explicit` set scores 1.0.)
  - [ ] Krippendorff α ≥ 0.75 (currently 0.52)
  - [ ] Human blind test completed
  - [ ] Results documented in `eval/manifests/eval-v0.1.0.json`

- [ ] **P7: Professional review complete**
  - [ ] Screenwriter calibration done
  - [ ] Hard positive constructed
  - [ ] Discrimination pair accuracy measured
  - [ ] Sealed holdout executed
  - [ ] At least 1 promotion decision from human review

- [ ] **All judge routes operational**
  - [ ] DeepSeek (generator): ✅
  - [ ] Qwen (judge): ✅
  - [ ] GLM/Zhipu (judge): ❌ (needs recharge)
  - [ ] OpenAI/Codex (judge): ✅
  - Minimum 3 judge families required

### ✅ Documentation

- [ ] CHANGELOG.md updated
  - All changes since last version listed
  - Breaking changes highlighted
  - Migration guide (if needed)

- [ ] README.md accurate
  - Features match implementation
  - Installation steps verified
  - Quick start works on clean machine

- [ ] HANDOFF.md updated
  - Current status accurate
  - Known issues documented
  - Date and version updated

- [ ] ROADMAP.md status accurate
  - Phase completion status correct
  - Exit conditions updated
  - Dependency graph current

- [ ] API documentation complete
  - All public APIs documented
  - Examples provided
  - Breaking changes noted

### ✅ Security

- [ ] SECURITY_REVIEW.md updated
  - No open blockers remain
  - All controls verified
  - Audit date current

- [ ] Credentials encrypted
  - Windows Credential Manager working
  - No plaintext keys in logs
  - Diagnostics properly redacted

- [ ] Dependency audit clean
  ```powershell
  cargo audit
  # No critical vulnerabilities
  ```

- [ ] License compliance
  - distribution-license-policy-v1.json up to date
  - All dependencies reviewed
  - campaign-muti-agent license verified

### ✅ Build & Package

- [ ] Version numbers updated
  - Cargo.toml (all crates)
  - apps/desktop/package.json
  - apps/desktop/src-tauri/tauri.conf.json
  - All versions match

- [ ] Changelog entry for this version
  - Date added
  - All changes listed
  - Release notes complete

- [ ] Clean build successful
  ```powershell
  cargo clean
  cargo build --workspace --release
  ```

- [ ] Desktop build successful
  ```powershell
  cd apps/desktop
  npm run tauri build
  ```

- [ ] Installer created
  - MSI: ✅
  - NSIS: ✅
  - Checksums generated

- [ ] Authenticode signing (optional for public release)
  - [ ] Certificate valid
  - [ ] Installer signed
  - [ ] Signature verified

### ✅ Release Artifacts

- [ ] Git tag created
  ```powershell
  git tag -a v0.1.0 -m "Release v0.1.0"
  ```

- [ ] Release notes written
  - Highlights
  - Breaking changes
  - Migration guide
  - Known issues

- [ ] Checksums file created
  ```powershell
  # SHA256 of all installers
  Get-FileHash *.exe, *.msi | Format-List
  ```

- [ ] Build provenance documented
  - Rust version: 1.88.0
  - Node version: 22.14.0
  - Python version: 3.12.10
  - Build date
  - Git commit hash
  - Builder identity

---

## Release Process

### Step 1: Pre-Release Verification

```powershell
# Run all checks
.\scripts\pre-release-check.ps1

# Review output
# All checks must pass
```

### Step 2: Update Version Numbers

```powershell
# Update all Cargo.toml files
# Example: 0.1.0-alpha.1 → 0.1.0
(Get-Content Cargo.toml) -replace '0.1.0-alpha.1', '0.1.0' | Set-Content Cargo.toml

# Update package.json
# Update tauri.conf.json
```

### Step 3: Update Documentation

```powershell
# Update CHANGELOG.md
# Add release date
# Review all changes

# Update HANDOFF.md
# Set status: release
# Update date

# Update version references in docs
```

### Step 4: Build Release

```powershell
# Clean build
cargo clean
cd apps/desktop
npm run tauri build

# Verify installers created
ls src-tauri/target/release/bundle/
```

### Step 5: Sign Installers (Optional)

```powershell
# If Authenticode certificate available
signtool sign /f certificate.pfx /p password /t http://timestamp.digicert.com installer.exe
```

### Step 6: Generate Checksums

```powershell
# Generate SHA256 for all installers
Get-FileHash *.exe, *.msi -Algorithm SHA256 | 
  Select-Object Algorithm, Hash, Path | 
  Format-List | 
  Out-File checksums.txt
```

### Step 7: Create Git Tag

```powershell
git add .
git commit -m "Release v0.1.0"
git tag -a v0.1.0 -m "Release v0.1.0

Highlights:
- First stable release
- P5-P10 complete
- Human review passed
- Production ready

See CHANGELOG.md for full details."

git push origin main
git push origin v0.1.0
```

### Step 8: GitHub Release

1. Go to: https://github.com/dgy-github/microcodex-short-drama-studio/releases/new
2. Choose tag: v0.1.0
3. Title: "v0.1.0 - First Stable Release"
4. Description: Copy from release notes
5. Upload artifacts:
   - microcodex-short-drama-studio_0.1.0_x64-setup.exe
   - microcodex-short-drama-studio_0.1.0_x64.msi
   - checksums.txt
6. If pre-release: ☐ (unchecked for stable)
7. Publish release

### Step 9: Post-Release

- [ ] Verify GitHub release visible
- [ ] Test download links
- [ ] Update project website (if exists)
- [ ] Announce on relevant channels
- [ ] Monitor for issues

---

## Rollback Procedure

If critical issues found after release:

1. **Immediate**: Pull release from GitHub (make private)
2. **Document**: Record issue in `docs/incidents/`
3. **Fix**: Create hotfix branch
4. **Test**: Run full checklist again
5. **Re-release**: New patch version (e.g., 0.1.1)

---

## Version Numbering

Following Semantic Versioning (semver):

- **MAJOR** (X.0.0): Breaking changes
- **MINOR** (0.X.0): New features, backward compatible
- **PATCH** (0.0.X): Bug fixes, backward compatible

**Alpha/Beta/RC**:
- `0.1.0-alpha.1`: Early development
- `0.1.0-beta.1`: Feature complete, testing
- `0.1.0-rc.1`: Release candidate
- `0.1.0`: Stable release

---

## Current Status

**Target Version**: 0.1.0 (first stable)
**Current Version**: 0.1.0-alpha.1
**Status**: 🔴 NOT READY

**Blockers**:
1. ❌ P1 exit condition not met (seeded_defect_detection = 0.0 over a single seeded pair)
2. ❌ P7 professional review not complete
3. ❌ P10 Clean VM test not executed
4. ❌ GLM judge route needs recharge

**Next Steps**:
1. Complete human blind test (P1)
2. Recruit and calibrate screenwriters (P7)
3. Execute Clean VM acceptance test (P10)
4. Recharge GLM route

---

## Notes

- **Never skip blockers**: P1, P7, P10 are mandatory
- **Document everything**: All test results must be written
- **Reproducible**: Anyone should be able to verify the build
- **Honest**: If something is advisory/non-promotable, say so

---

**Last Updated**: 2026-08-10  
**Maintained By**: Project Owner  
**Review Before Each Release**: Yes
