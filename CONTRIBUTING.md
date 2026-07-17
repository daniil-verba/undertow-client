# Contributing to Undertow Client

Thank you for considering contributing to the project! We welcome any improvements — from fixing typos to adding new features.

**Important:** Before you start, please read [README.md](README.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

[Русский](CONTRIBUTING.ru.md) | **English**

---

## 📋 Table of Contents

1. [How to Start](#how-to-start)
2. [Project Structure](#project-structure)
3. [Commit Rules](#commit-rules)
4. [Working with Branches](#working-with-branches)
5. [Creating a Pull Request](#creating-a-pull-request)
6. [Code Review](#code-review)
7. [Testing](#testing)
8. [Code Style](#code-style)
9. [Documentation](#documentation)
10. [Frequently Asked Questions](#frequently-asked-questions)

---

## 🚀 How to Start

### 1. Fork the Repository
Click the **Fork** button in the top right corner on GitHub.

### 2. Clone Your Fork
```bash
git clone https://github.com/your-username/undertow-client.git
cd undertow-client
```

### 3. Add Upstream (Original Repository)
```bash
git remote add upstream https://github.com/daniil-verba/undertow-client.git
git fetch upstream
```

### 4. Install Dependencies
```bash
# Make sure you have Rust installed
rustc --version  # Should be 1.70+

# Install necessary tools
cargo install cargo-watch cargo-deny cargo-tarpaulin
```

### 5. Verify Everything Works
```bash
cargo build
cargo test
cargo run
```

---

## 📁 Project Structure

```
Undertow-Client/
├── assets/                         # Media files for documentation
│   └── screenshot.png              # Screenshot of the TUI interface
├── src/
│   ├── ui/                         # TUI components (user interface)
│   │   ├── app.rs                  # Main application state
│   │   ├── mod.rs                  # Entry point to the ui module
│   │   └── widgets.rs              # TUI widgets (chat, peer list, status)
│   ├── main.rs                     # Application entry point (TUI launch)
├── Cargo.toml                      # Project manifest (dependencies, metadata)
├── Cargo.lock                      # Dependency lock file
├── CHANGELOG.md                    # Changelog (English)
├── CHANGELOG.ru.md                 # Changelog (Russian)
├── CODE_OF_CONDUCT.md              # Code of Conduct (English)
├── CODE_OF_CONDUCT.ru.md           # Code of Conduct (Russian)
├── CONTRIBUTING.md                 # Contributing guide (English)
├── CONTRIBUTING.ru.md              # Contributing guide (Russian)
├── LICENSE                         # MIT License
├── README.md                       # Project description (English)
└── README.ru.md                    # Project description (Russian)
```

---

## 📖 Key Modules Overview

| Module | Purpose | Key Files |
| :--- | :--- | :--- |
| **`ui`** | **TUI interface**: chat rendering, peer list, Beacon connection status. | `app.rs`, `widgets.rs` |
| **`main.rs`** | **Entry point**: application initialization, TUI loop startup, network connection. | `main.rs` |

---

## 🔧 Module Relationships

- **`main.rs`** — launches the application, creates an `App` instance, and starts the TUI loop.
- **`ui/app.rs`** — manages application state (messages, peer list, Beacon connection).
- **`ui/widgets.rs`** — renders individual interface elements (chat, peer list, status bar).
- **`undertow-protocol`** — used as an external dependency for networking, cryptography, and protocol logic.

---

## 📦 Dependencies

| Dependency | Purpose |
| :--- | :--- |
| **`undertow-protocol`** | Core library (networking, cryptography, PeerId). |
| **`ratatui`** | TUI framework. |
| **`crossterm`** | Cross-platform terminal handling. |
| **`tokio`** | Asynchronous runtime for networking. |

---

## 📝 Commit Rules

We follow [Conventional Commits](https://www.conventionalcommits.org/).

### Format
```bash
<type>(<scope>): <description>

[detailed description]

[task reference]
```

### Commit Types

| Type | When to use |
|-----|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code formatting |
| `refactor` | Refactoring (no logic change) |
| `perf` | Performance improvement |
| `test` | Adding tests |
| `chore` | Maintenance tasks (dependencies, build) |
| `ci` | CI/CD configuration |
| `revert` | Revert changes |

### Examples

```bash
# New feature
git commit -m "feat(network): add IPv6 support"

# Bug fix
git commit -m "fix(ui): fix crash on command input"

# Documentation update
git commit -m "docs(readme): update installation instructions"

# Refactoring
git commit -m "refactor(app): simplify event handling"
```

### ❌ Bad Commit Examples

```bash
git commit -m "fix"           # ❌ No description
git commit -m "oops broke"    # ❌ Uninformative
git commit -m "updated stuff" # ❌ Too vague
```

---

## 🌳 Working with Branches

### Branching Strategy

```text
main  →  stable version (releases only)
  │
  └── dev  →  main development
       │
       ├── feature/feature-name   # New features
       ├── fix/bug-name           # Bug fixes
       └── docs/name              # Documentation
```

### Rules

1. **Always branch from `dev`:**
   ```bash
   git checkout dev
   git pull origin dev
   git checkout -b feature/your-feature
   ```

2. **Name branches meaningfully:**
   - `feature/add-p2p` — new feature
   - `fix/connection-timeout` — bug fix
   - `docs/update-readme` — documentation

3. **One branch = one task**

4. **Delete branches after merging**

### Example Workflow

```bash
# 1. Create a feature branch
git checkout dev
git checkout -b feature/add-p2p

# 2. Work and commit
git add .
git commit -m "feat(network): add P2P connection"
git commit -m "test(network): add P2P tests"

# 3. Push to your fork
git push origin feature/add-p2p

# 4. Create a Pull Request on GitHub
```

---

## 🔄 Creating a Pull Request

### Pre-PR Checklist

- [ ] Branch is from **up-to-date `dev`**
- [ ] All commits follow [Conventional Commits](#commit-rules)
- [ ] Code passes `cargo fmt` and `cargo clippy`
- [ ] All tests pass: `cargo test`
- [ ] Tests added for new functionality
- [ ] Documentation updated (if needed)
- [ ] Branch pushed to your fork

### Creating the PR

1. Go to your fork on GitHub
2. Click **"Compare & pull request"**
3. Select:
   - **base:** `daniil-verba/undertow-client:dev`
   - **compare:** `your-username/undertow-client:feature/your-feature`
4. Fill in the PR template:

```markdown
## Description
Briefly describe what you've done.

## Related Issues
Closes #42 (if applicable)

## Change Type
- [ ] New feature (feat)
- [ ] Bug fix (fix)
- [ ] Documentation (docs)
- [ ] Refactoring (refactor)
- [ ] Tests (test)

## How to Test?
1. Step 1
2. Step 2

## Checklist
- [ ] Code passes `cargo fmt`
- [ ] Code passes `cargo clippy`
- [ ] Tests pass `cargo test`
- [ ] Documentation updated
```

### Good PR Example

```markdown
## Description
Added IPv6 support for P2P connections.

## Related Issues
Closes #42

## Change Type
- [x] New feature (feat)

## How to Test?
1. cargo run -- --ipv6
2. Connect to a peer via IPv6 address

## Checklist
- [x] Code passes `cargo fmt`
- [x] Code passes `cargo clippy`
- [x] Tests pass `cargo test`
- [x] Documentation updated
```

---

## 👀 Code Review

After creating the PR, a maintainer will review it.

### What We Check

1. **Code style compliance**
2. **Test coverage**
3. **No regressions**
4. **Documentation quality**

### If Changes Are Requested

```bash
# Make changes in the same branch
git add .
git commit -m "fix: review fixes"
git push origin feature/your-feature
```

The PR will update automatically after push.

---

## 🧪 Testing

### Running All Tests

```bash
cargo test
```

### Running a Specific Test

```bash
cargo test test_name
```

### Running with Log Output

```bash
cargo test -- --nocapture
```

### Code Coverage

```bash
cargo tarpaulin --ignore-tests
```

### Test Requirements

- **New features** → must have tests
- **Bug fixes** → must include regression tests
- **Refactoring** → tests must pass without changes

---

## 🎨 Code Style

### Formatting

```bash
# Auto-format
cargo fmt

# Check formatting
cargo fmt --check
```

### Linting

```bash
# Run linter
cargo clippy

# Auto-fix warnings
cargo clippy --fix
```

### Naming Conventions

| Type | Rule | Example |
|------|------|---------|
| Variables | `snake_case` | `peer_count` |
| Functions | `snake_case` | `add_peer()` |
| Structs | `PascalCase` | `PeerInfo` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES` |
| Modules | `snake_case` | `network.rs` |

### Comments

```rust
/// Short function description
/// 
/// # Arguments
/// - `arg1` - description
/// - `arg2` - description
/// 
/// # Example
/// ```
/// let result = my_function(1, 2);
/// ```
pub fn my_function(arg1: u32, arg2: u32) -> u32 {
    // Internal comment for complex logic
    arg1 + arg2
}
```

---

## 📚 Documentation

### Updating README

If your change affects:
- New commands
- Configuration changes
- New requirements

Update `README.md` (and `README.ru.md` if present).

### Updating CHANGELOG

1. For each release, create a new section with the version and date.
2. Sort changes by importance: `Added` > `Changed` > `Fixed` > `Deprecated` > `Removed` > `Security`.
3. Use `Unreleased` to collect all changes already in the `dev` branch but not yet released.
4. After a release, move everything from `Unreleased` to a new version section.
5. Describe each change in one line: `- Change description (#issue-number)`

### Examples of Good Entries

```markdown
### Added
- Added IPv6 support for P2P connections (#42)
- Added `--config` CLI argument for custom config path (#15)

### Fixed
- Fixed panic on Beacon disconnect during transmission (#37)
- Fixed message duplication bug in Relay (#28)

### Changed
- Updated `tokio` dependency from 1.30 to 1.35 (#41)
```

---

## ❓ Frequently Asked Questions

### "I'm new to Rust. Can I contribute?"
✅ **YES!** We welcome beginners. Look for issues tagged `good-first-issue`.

### "Can I add a new feature without an Issue?"
✅ It's better to create an Issue first to discuss if the feature is needed.

### "What if I get stuck?"
Create a PR with `[WIP]` (Work In Progress) in the title and ask your question.

### "How often should I update my fork?"
```bash
git fetch upstream
git checkout dev
git merge upstream/dev
git push origin dev
```

---

## 📞 Contact

- **GitHub Issues:** [Create Issue](https://github.com/daniil-verba/undertow-client/issues)
- **Discord:** [Discord Link](https://discord.gg/your-invite-link)

---

**Thank you for contributing to Undertow Protocol!** ❤️
