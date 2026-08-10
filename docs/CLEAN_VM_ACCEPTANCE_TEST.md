# Clean VM Acceptance Test

**Purpose**: Verify that the product can be installed, configured, and used on a clean Windows machine.

**Status**: 🔴 P10 Exit Condition - NOT YET EXECUTED

**Target**: Before claiming production readiness

---

## Prerequisites

- Clean Windows 10+ VM (no dev tools pre-installed)
- Internet connection
- At least 8GB RAM, 20GB disk space

---

## Phase 1: Base Installation

### 1.1 Install Prerequisites

```powershell
# Install Chocolatey (package manager)
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

# Install Git
choco install git -y

# Install Rust 1.88.0
choco install rust --version=1.88.0 -y

# Install Node.js 22.14.0
choco install nodejs --version=22.14.0 -y

# Install Python 3.12.10
choco install python --version=3.12.10 -y

# Refresh environment
refreshenv
```

**Verification**:
```powershell
git --version          # Should output: git version 2.x.x
rustc --version        # Should output: rustc 1.88.0
node --version         # Should output: v22.14.0
python --version       # Should output: Python 3.12.10
```

**Exit Criteria**: All 4 tools installed and versions match

---

## Phase 2: Clone and Setup

### 2.1 Clone Repository

```powershell
cd $env:USERPROFILE
mkdir github
cd github
git clone https://github.com/dgy-github/microcodex-short-drama-studio.git
cd microcodex-short-drama-studio
```

**Verification**:
```powershell
Test-Path README.md     # Should return True
Test-Path Cargo.toml    # Should return True
```

**Exit Criteria**: Repository cloned successfully

### 2.2 Setup Python Environment

```powershell
# Create virtual environment
python -m venv .venv

# Activate
.\.venv\Scripts\Activate.ps1

# Install dependencies
pip install -e sidecar
```

**Verification**:
```powershell
.\.venv\Scripts\python.exe -c "import campaign; print('OK')"
# Should output: OK
```

**Exit Criteria**: Python dependencies installed, campaign module importable

### 2.3 Initialize Project

```powershell
python scripts/init_project.py --check
# If uninitialized:
python scripts/init_project.py --name "MicrocodeX Short Drama Studio"
```

**Verification**:
```powershell
python scripts/init_project.py --check
# Should output: All checks passed
```

**Exit Criteria**: Project initialized successfully

---

## Phase 3: Build and Test

### 3.1 Build Rust Workspace

```powershell
cargo build --workspace --all-features
```

**Verification**:
```powershell
# Check build artifacts
Test-Path target/debug/    # Should return True
```

**Exit Criteria**: Workspace builds without errors

**Estimated Time**: 5-10 minutes (first build)

### 3.2 Run Tests

```powershell
# Core tests
cargo test --workspace --all-features
# Expected: XX tests passed

# Desktop tests
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
# Expected: 19 tests passed (or current count)

# Python sidecar tests
python -m unittest discover -s sidecar -p "test_*.py"
# Expected: 19 tests passed

# Python eval tests
python -m unittest discover -s eval/tools -p "test_*.py"
# Expected: XX tests passed
```

**Exit Criteria**: All test suites pass with 0 failures

**Estimated Time**: 10-15 minutes

### 3.3 Code Quality Checks

```powershell
# Format check
cargo fmt --all -- --check
# Should output: No formatting issues

# Lint check
cargo clippy --workspace --all-targets --all-features -- -D warnings
# Should output: No clippy warnings

# Project integrity
python scripts/init_project.py --check
# Should output: All checks passed
```

**Exit Criteria**: All checks pass

---

## Phase 4: Install Desktop Application

### 4.1 Build Installer

```powershell
cd apps/desktop
npm install
npm run tauri build
```

**Verification**:
```powershell
# Check installer created
$installerPath = "src-tauri/target/release/bundle/nsis/microcodex-short-drama-studio_0.1.0-alpha.1_x64-setup.exe"
Test-Path $installerPath   # Should return True
```

**Exit Criteria**: Installer built successfully

**Estimated Time**: 10-20 minutes

### 4.2 Install Application

```powershell
# Navigate to installer
cd src-tauri/target/release/bundle/nsis/

# Run installer
Start-Process ".\microcodex-short-drama-studio_0.1.0-alpha.1_x64-setup.exe" -Wait
```

**Manual Steps**:
1. Click through installer wizard
2. Accept license (if present)
3. Choose installation directory
4. Complete installation

**Verification**:
```powershell
# Check installation directory
$installPath = "$env:ProgramFiles\microcodex-short-drama-studio"
Test-Path $installPath   # Should return True

# Check desktop shortcut created
Test-Path "$env:PUBLIC\Desktop\MicrocodeX Short Drama Studio.lnk"
```

**Exit Criteria**: Application installed, shortcuts created

---

## Phase 5: First Run Configuration

### 5.1 Launch Application

```powershell
Start-Process "$env:ProgramFiles\microcodex-short-drama-studio\microcodex-short-drama-studio.exe"
```

**Manual Steps**:
1. Application window opens
2. First-run wizard appears (if implemented)
3. No crash or error dialogs

**Exit Criteria**: Application launches successfully

### 5.2 Configure Providers

**Manual Steps**:
1. Navigate to Settings → Provider Configuration
2. Add DeepSeek credentials:
   - API Key: `sk-...` (test key)
   - Endpoint: Default or custom
3. Add Bailian (百炼) credentials:
   - API Key: `...`
   - Endpoint: Default or custom
4. Run Health Check for each provider

**Verification**:
- DeepSeek: ✅ Healthy
- Bailian: ✅ Healthy

**Exit Criteria**: Both providers configured and healthy

---

## Phase 6: Complete Story Generation

### 6.1 Create Story Project

**Manual Steps**:
1. Click "New Story Project"
2. Fill in details:
   - Premise: "两名维修工必须在商场开门前修好同一部故障电梯"
   - Genre: Family
   - Episodes: 6
   - Duration: 2 min/episode
   - Budget: max_tokens=180000, max_cny_fen=1200
3. Click "Create"

**Exit Criteria**: Project created successfully

### 6.2 Start Story Generation

**Manual Steps**:
1. Click "Start Generation"
2. Monitor progress:
   - Task progress bar
   - Token usage counter
   - Estimated time remaining

**Verification**:
- No errors displayed
- Progress updates regularly
- Task count increments (1/17, 2/17, ...)

**Exit Criteria**: All 17 tasks complete

**Estimated Time**: 10-30 minutes (depends on provider speed)

### 6.3 Review Generated Story

**Manual Steps**:
1. Click on completed story in library
2. Open full story reader
3. Check:
   - ✅ 6 episodes present
   - ✅ Character cards displayed
   - ✅ Dialogue has speaker labels
   - ✅ Subtext annotations visible
   - ✅ Scene descriptions readable

**Exit Criteria**: Story package is complete and readable

---

## Phase 7: Export Story

### 7.1 Approve Story

**Manual Steps**:
1. In story viewer, click "Approve"
2. Confirm approval dialog
3. Check approval status changes to "Approved"

**Exit Criteria**: Story marked as approved

### 7.2 Export Package

**Manual Steps**:
1. Click "Export"
2. Choose export location
3. Wait for export to complete

**Verification**:
```powershell
# Check exported file
$exportPath = "$env:USERPROFILE\Downloads\story-package-*.json"
Test-Path $exportPath   # Should return True

# Validate JSON
$json = Get-Content $exportPath | ConvertFrom-Json
$json.schema           # Should be: story-package/v1
$json.episodes.Count   # Should be: 6
```

**Exit Criteria**: Valid story-package/v1 JSON exported

---

## Phase 8: Upgrade Test

### 8.1 Build New Version

```powershell
# Bump version in Cargo.toml (simulate)
# For test, just rebuild same version
cd $env:USERPROFILE\github\microcodex-short-drama-studio\apps\desktop
npm run tauri build
```

**Exit Criteria**: New installer built

### 8.2 Upgrade Installation

**Manual Steps**:
1. Run new installer
2. Should detect existing installation
3. Choose "Upgrade"
4. Complete upgrade

**Verification**:
```powershell
# Check version (in app or file properties)
(Get-Item "$env:ProgramFiles\microcodex-short-drama-studio\microcodex-short-drama-studio.exe").VersionInfo.FileVersion
```

**Exit Criteria**: 
- Upgrade completes without errors
- Previous stories still accessible
- Configuration retained

---

## Phase 9: Rollback Test

### 9.1 Uninstall Current Version

**Manual Steps**:
1. Go to Windows Settings → Apps
2. Find "MicrocodeX Short Drama Studio"
3. Click Uninstall
4. Complete uninstall

**Verification**:
```powershell
Test-Path "$env:ProgramFiles\microcodex-short-drama-studio"
# Should return False
```

**Exit Criteria**: Application uninstalled cleanly

### 9.2 Reinstall Previous Version

**Manual Steps**:
1. Run previous installer
2. Install to same location
3. Launch application

**Verification**:
- Application launches
- Previous stories should be accessible (if data not removed)
- Configuration may need re-entry (depending on design)

**Exit Criteria**: Previous version works

---

## Phase 10: Clean Uninstall

### 10.1 Uninstall Application

**Manual Steps**:
1. Uninstall via Windows Settings
2. Choose "Remove all data" (if option exists)

**Verification**:
```powershell
# Check directories removed
Test-Path "$env:ProgramFiles\microcodex-short-drama-studio"    # False
Test-Path "$env:APPDATA\microcodex-short-drama-studio"         # False (optional)
Test-Path "$env:LOCALAPPDATA\microcodex-short-drama-studio"    # False (optional)
```

**Exit Criteria**: All application files removed

---

## Final Checklist

- [ ] Phase 1: Prerequisites installed ✅
- [ ] Phase 2: Repository cloned and setup ✅
- [ ] Phase 3: All tests pass ✅
- [ ] Phase 4: Installer built ✅
- [ ] Phase 5: Application configured ✅
- [ ] Phase 6: Story generated successfully ✅
- [ ] Phase 7: Story exported ✅
- [ ] Phase 8: Upgrade successful ✅
- [ ] Phase 9: Rollback successful ✅
- [ ] Phase 10: Clean uninstall ✅

---

## Success Criteria

✅ **P10 Exit Condition Met**: A clean machine can install, configure, complete and export a story job, upgrade safely, and reproduce the release evidence.

---

## Failure Scenarios

If any phase fails, document:

1. **Phase number and step**
2. **Error message** (screenshot + text)
3. **System state** (installed versions, disk space, etc.)
4. **Reproduction steps**
5. **Workaround** (if any)

Record failures in: `docs/acceptance-test-results/YYYYMMDD-clean-vm-test.md`

---

## Notes

- **Estimated Total Time**: 2-3 hours (excluding provider wait time)
- **VM Snapshot**: Take snapshot before starting, can reset if needed
- **Credentials**: Use test credentials, not production
- **Network**: Some steps require internet, note if offline test is needed
- **Automation**: Future improvement - automate as much as possible

---

**Last Updated**: 2026-08-10  
**Status**: Script created, awaiting execution  
**Tracking**: IMPROVEMENT_PLAN.md Phase 3.1
