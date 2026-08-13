# MicrocodeX Short Drama Studio

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.88.0-orange.svg)](https://www.rust-lang.org/)
[![Python](https://img.shields.io/badge/python-3.12.10-blue.svg)](https://www.python.org/)
[![Status](https://img.shields.io/badge/status-alpha-red.svg)](https://github.com/dgy-github/microcodex-short-drama-studio)

> ⚠️ **ALPHA STATUS - ADVISORY ONLY**  
> This project is in **alpha stage**. All generated content is marked as **advisory/non-promotable** and has not completed human blind review. Use for experimentation and learning only. See [Current Limitations](#️-current-limitations) for details.

---

[GitHub repository](https://github.com/dgy-github/microcodex-short-drama-studio)

Windows-first short-drama story development workspace. It turns a short Chinese
idea into multiple story candidates, character and episode plans, reviewed
sample scenes, and a versioned story package.

## Architecture

- Rust workspace: trusted product contracts, event protocol, storage/provider seams, and evaluation helpers.
- Campaign sidecar: multi-agent DAG orchestration, routing, review, recovery, and event replay.
- Tauri + Svelte desktop: operator UI; added after the M0 runtime contract is stable.
- nanocodex: offline derivation of candidate `SKILL.md` files from licensed human revision traces.

Long-running communication follows one rule:

```text
asynchronous command -> append-only event -> SSE -> durable replay
```

Quality is governed by **two separate design systems**. Offline evaluation is the
instrument that gates versions; online policy is the strategy that makes
in-run decisions. The dependency runs one way — policy configuration is promoted
through evaluation, and evaluation criteria never adapt to production outcomes.

| Document | Version |
| --- | --- |
| [Offline evaluation v1](docs/STORY_EVAL_V1.md) — runnable now, no professional panel | `story-eval-offline/v1.0.0` |
| [Adversarial set construction](docs/STORY_EVAL_ADVERSARIAL.md) | `story-eval-adversarial/v1.0.0` |
| [Online policy design](docs/ONLINE_POLICY_DESIGN.md) — in-run weighting | `online-policy-design/v1.0.0` |
| [Story evaluation target contract](docs/STORY_EVAL_DESIGN.md) — professional-panel end state | `story-eval-target/v1.0.0` |
| [Story multi-agent design](docs/STORY_MULTI_AGENT_DESIGN.md) | `story-multi-agent/v1.0.0` |
| [Architecture decisions](docs/ARCHITECTURE.md) | `architecture/v1.0.0` |

Version bump rules, artifact bindings, and current freeze state are in
[docs/VERSIONS.md](docs/VERSIONS.md). Nothing is frozen yet.

Phase sequencing and exit criteria are in [docs/ROADMAP.md](docs/ROADMAP.md);
where the last session stopped is in [HANDOFF.md](HANDOFF.md).

## Current milestone

P5-P10 non-human implementation is present as an advisory desktop release
candidate. It generates and reviews story packages, but does not claim stable
release or model/prompt promotion while the blockers in `CHANGELOG.md` remain.

## Quick Start

### Prerequisites

- **Rust**: 1.88.0 (see `rust-toolchain.toml`)
- **Node.js**: 22.14.0 (see `.nvmrc`)
- **Python**: 3.12.10 (see `.python-version`)
- **Windows 10+** with Windows Credential Manager
- **Git** for dependency installation

### First-Time Setup

1. **Clone the repository**
   ```bash
   git clone https://github.com/dgy-github/microcodex-short-drama-studio.git
   cd microcodex-short-drama-studio
   ```

2. **Set up Python environment**
   ```powershell
   python -m venv .venv
   .\.venv\Scripts\python.exe -m pip install -e sidecar
   .\.venv\Scripts\python.exe -m pip install -r scripts/requirements.txt
   .\.venv\Scripts\python.exe -m pip install -r eval/tools/requirements.txt
   ```

   The three install steps cover, respectively, the campaign sidecar runtime
   (`aiohttp` + `campaign-muti-agent`), the toolchain scripts under `scripts/`
   (`PyYAML`), and the evaluation tools under `eval/tools/` (`PyYAML` +
   `jsonschema`). Alternatively, run `python scripts/setup_dev_environment.py`
   to provision the venv and install all three dependency groups in one step.

3. **Initialize project**
   ```powershell
   .\.venv\Scripts\python.exe scripts/init_project.py --check
   # If uninitialized:
   .\.venv\Scripts\python.exe scripts/init_project.py --name "MicrocodeX Short Drama Studio"
   ```

4. **Verify installation**
   ```powershell
   # Test Rust workspace
   cargo test --workspace --all-features
   
   # Test desktop app
   cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
   
   # Test Python components
   .\.venv\Scripts\python.exe -m unittest discover -s sidecar -p "test_*.py"
   .\.venv\Scripts\python.exe -m unittest discover -s eval/tools -p "test_*.py"
   ```

### Development Workflow

Before starting work, read:
1. `docs/ROADMAP.md` - Phase sequencing and exit criteria
2. `HANDOFF.md` - Current project status
3. `docs/SECURITY_REVIEW.md` - Security controls
4. `AGENTS.md` - Development rules

For cross-layer features, follow the G0-G7 gates in `docs/development/WORKFLOW.md`.

Query `docs/project-memory/PROJECT_REGISTRY.yaml` before adding code or interfaces.

### Code Quality Checks

Run before committing:

```powershell
# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Test
cargo test --workspace --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
.\.venv\Scripts\python.exe -m unittest discover -s sidecar -p "test_*.py"
.\.venv\Scripts\python.exe -m unittest discover -s eval/tools -p "test_*.py"

# Project integrity
.\.venv\Scripts\python.exe scripts/init_project.py --check
```

### Building Desktop App

```powershell
cd apps/desktop
npm install
npm run tauri build
```

The installer will be in `apps/desktop/src-tauri/target/release/bundle/`.

---

## 🎯 Project Highlights

### Architecture
- **Rust** (6 crates, ~5,767 lines): Core contracts, storage, providers, evaluation
- **Python** sidecar (~8,008 lines): Multi-agent DAG orchestration via [campaign-muti-agent](https://github.com/dgy-github/campaign-muti-agent)
- **Tauri 2.8 + Svelte 5**: Desktop application with Windows Credential Manager integration
- **Event Sourcing**: Append-only event log with SSE resume capability

### Quality Practices
- **Fail-closed design**: All critical paths fail safely
- **Form-agnostic abstraction**: Content forms are configuration-driven
- **Dual quality governance**: Offline evaluation (story-eval) vs online policy (story-policy)
- **Security**: Encrypted credentials, diagnostic redaction, zero unsafe code in core
- **Testing**: 82+ tests across Rust and Python components
- **Documentation**: Complete design docs, ADRs, security review, and independent audit

### Proven Capabilities
- ✅ **Real paid run successful** (2026-07-30): 17/17 tasks, 6 episodes, 154K tokens
- ✅ Fixed 17-task DAG workflow with DeepSeek generation + Bailian review
- ✅ Desktop app with story reader (comic-style UI)
- ✅ Immutable revision history, approval workflow, export capability
- ✅ MSI/NSIS installers with bundled Python sidecar

---

## ⚠️ Current Limitations

**This is an alpha release. The following blockers prevent production use:**

### P1: Evaluation System Not Validated
- **Issue**: `seeded_defect_detection = 0.0` (target: 0.75)
- **Impact**: Cannot verify that judges can distinguish degraded versions
- **Status**: Human blind test deferred until first end-to-end artifact completed

### P7: Professional Review Missing
- **Issue**: No professional screenwriter validation
- **Impact**: LLM judges cannot promote candidates alone
- **Status**: Recruitment and calibration not started

### P10: Clean VM Acceptance Not Executed
- **Issue**: Install→Configure→Story→Export→Upgrade→Rollback not verified
- **Impact**: Real-world usability uncertain
- **Status**: Test script ready in `docs/CLEAN_VM_ACCEPTANCE_TEST.md`

**All output must be marked `advisory/non-promotable` until these are resolved.**

See [PROJECT_STATUS_REPORT.md](PROJECT_STATUS_REPORT.md) for detailed analysis.

---

## 🗺️ Roadmap

**Current Phase**: P5-P10 engineering complete, awaiting validation

**Completed**:
- ✅ P3b: Advisory runtime with real paid run
- ✅ P5: Usable desktop application
- ✅ P6: Revision and approval workflow
- ✅ P8: 8 genre packs (family, suspense, urban romance, etc.)
- ✅ P9: Production reliability (crash recovery, backup/restore)
- ✅ P10: Windows packaging (MSI/NSIS)

**Blocked on External Dependencies**:
- 🔴 P1: Judge calibration (needs human blind test)
- 🔴 P7: Professional gate (needs screenwriter recruitment)
- 🟡 P10: Clean VM acceptance (needs execution)

**Future** (P11-P18):
- Expand evaluation set (30 → 120 cases)
- Skill derivation and promotion loop
- Additional content forms (knowledge/explainer, real-creator)
- Export to shootable screenplay formats

See [docs/ROADMAP.md](docs/ROADMAP.md) for complete phase sequencing.

---

## 📚 Documentation

- **[Quick Start](#quick-start)** - Get up and running
- **[TROUBLESHOOTING.md](TROUBLESHOOTING.md)** - Common issues and solutions
- **[PROJECT_STATUS_REPORT.md](PROJECT_STATUS_REPORT.md)** - Comprehensive project status
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System design and boundaries
- **[docs/ROADMAP.md](docs/ROADMAP.md)** - Phase sequencing and exit criteria
- **[docs/SECURITY_REVIEW.md](docs/SECURITY_REVIEW.md)** - Security controls
- **[HANDOFF.md](HANDOFF.md)** - Current development status
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - How to contribute

---

## 🤝 Contributing

Contributions are welcome! This is an alpha project with known limitations. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

**High-value contributions**:
1. **P10 Execution**: Run Clean VM acceptance test, report results
2. **Judge Improvements**: Improve prompt stability or propose alternative judges
3. **Test Coverage**: Add integration tests for desktop application
4. **Documentation**: Improve examples, tutorials, or translations
5. **Bug Reports**: File detailed issues with reproduction steps

---

## 📊 Project Statistics

- **Code**: ~15,000 lines (Rust + Python + Svelte)
- **Tests**: 82+ tests
- **Documentation**: 20+ markdown files
- **Development Time**: Several months (solo developer)
- **Quality Rating**: A- (excellent for solo project)

**Unique Achievement**: Single-developer project with team-level engineering maturity, including independent audit, formal ADRs, and fail-closed design throughout.

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

**Third-party dependencies**:
- Pinned orchestration: [campaign-muti-agent@1d93571](https://github.com/dgy-github/campaign-muti-agent) (MIT)
- See `config/distribution-license-policy-v1.json` for complete inventory

---

## 🙏 Acknowledgments

- **Claude Opus 5**: Independent audit (2026-07-29) identifying 12 improvement areas
- **Claude Fable 5**: Code quality assessment and documentation improvements (2026-08-10)
- Rust, Python, Tauri, and Svelte communities

---

## ⚖️ Disclaimer

This is an **experimental alpha release** for research and learning purposes. All generated story content is **advisory/non-promotable** and has not undergone professional human review. Do not use for production without completing P1, P7, and P10 validation gates.

For questions, issues, or discussions, please use [GitHub Issues](https://github.com/dgy-github/microcodex-short-drama-studio/issues) or [Discussions](https://github.com/dgy-github/microcodex-short-drama-studio/discussions).
