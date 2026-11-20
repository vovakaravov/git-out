<p align="center">
  <img src="assets/git-out.svg" alt="Git Out" width="88" height="88" />
</p>

<h1 align="center">Git Out</h1>

<p align="center">
  <strong>"I'm not paying for GitKraken."</strong>
</p>

<p align="center">
  A fast, zero-bloat native Git &amp; GitHub desktop client built in Rust + <a href="https://gpui.rs">GPUI</a> and powered by <code>git</code> and <code>gh</code>.
</p>

---

## Why Git Out?

- **Zero Electron. Zero WebView.** Rendered directly on the GPU using **GPUI** (`gpui-kit`)—the same UI stack behind Zed and Discord's native rewrite.
- **No subscriptions, no accounts, no telemetry.** Just the visual commit graph, chunky action buttons, and diff viewer you actually use every day.
- **Works with org-restricted GitHub repos.** Instead of registering yet another third-party OAuth app that your organization blocks, Git Out shells out directly to the official GitHub CLI (`gh`) you are already authenticated with.
- **Inherits your exact Git setup.** SSH keys, GPG signing, hooks, credential helpers, and `.gitconfig` aliases all work out of the box because `git` itself does the heavy lifting.

---

## Features

- **GPU-Rendered Commit Rail Graph**
  - Topological lane layout with smooth Bezier branch/merge curves, `HEAD` halos, and merge indicators.
  - Virtualized commit list with instant search/filtering by subject, author, branch/tag name, or SHA.
  - Live **PR & CI status badges** overlaid directly onto branches in the graph.
- **Chunky Action Toolbar & Sidebar**
  - One-click **Fetch**, **Pull** (`↓N`), **Push** (`↑N`, auto-sets upstream), **Branch**, **Stash**, **Pop**, and **Refresh**.
  - Collapsible sidebar for **Local** branches, **Remotes**, **Tags**, **Stashes** (Apply / Pop / Drop), and open **Pull Requests**.
- **Working Tree Staging & Commit Inspector**
  - Pinned **WIP** row at the top of the graph for full-file staging, unstaging, discarding, commit message + description composing, and amending.
  - Click any commit to **Checkout**, create a **Branch**, **Cherry-pick**, **Revert**, or **Reset** (`Soft` / `Mixed` / `Hard`), with **Unified** and **Split** diff views.
- **Multi-Repo Tabs**
  - Keep multiple repositories open in tabs with cached instant switching and session persistence.
- **Seamless `gh` GitHub Browser**
  - **PRs:** Filter (`Open` / `Merged` / `Closed` / `All`), view Markdown descriptions, CI check statuses, full review history (`Approved` / `Changes Requested` / `Commented`), **Files Changed** diffs, post comments, **Review…** (`gh pr review`), **Ready for Review** (`gh pr ready`), **Checkout PR**, **Create PR**, and **Merge…** (`Squash` / `Merge` / `Rebase`).
  - **Actions:** Browse workflow runs, inspect job & step statuses, view failing logs directly via `gh run view --log-failed`, and trigger **Rerun Failed Jobs**.
  - **Issues:** Browse and create issues, read Markdown bodies/comments, and one-click **Create Branch** (`gh issue develop --checkout`).
- **Live `.git` Watcher & Command Log**
  - Watches `.git/` so terminal or LLM-agent Git commands automatically refresh the UI.
  - Toggleable **Command Log** drawer (`Ctrl+\``) showing every `git` and `gh` invocation, timing, and output.

---

## Requirements

- **Git** (`git`)
- **GitHub CLI** (`gh`, already authenticated via `gh auth login`) — optional; local Git features work offline or without GitHub remotes
- **Rust toolchain** (`rustc` + `cargo`, Edition 2024)
- **Linux system libraries** (Wayland/X11 + Vulkan + fontconfig):
  ```bash
  # Arch Linux
  sudo pacman -S --needed base-devel pkgconf fontconfig freetype2 wayland libxkbcommon libxkbcommon-x11 libxcb openssl vulkan-icd-loader
  ```

---

## Build & Install

```bash
git clone https://github.com/vovakaravov/git-out.git
cd git-out
cargo build --release

# Put `gitout` on your PATH
mkdir -p ~/.local/bin
ln -sf "$(pwd)/target/release/gitout" ~/.local/bin/gitout
```

### Usage

```bash
# Open the repo in the current directory
gitout .

# Open a specific repository path
gitout /path/to/repo

# Launch and pick from recent / discovered repos in ~/
gitout
```

### Optional: Linux `.desktop` Entry (KDE / Wayland)

Git Out sets its Wayland `app_id` to `git-out` so desktop environments match the taskbar icon cleanly:

```bash
mkdir -p ~/.local/share/icons/hicolor/scalable/apps ~/.local/share/applications
cp assets/git-out.svg ~/.local/share/icons/hicolor/scalable/apps/git-out.svg

cat > ~/.local/share/applications/git-out.desktop <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=Git Out
GenericName=Git Client
Comment=Native GPU-accelerated Git & GitHub (gh) client
Exec=$HOME/.local/bin/gitout %f
Icon=$HOME/.local/share/icons/hicolor/scalable/apps/git-out.svg
Terminal=false
Categories=Development;RevisionControl;
Keywords=git;github;gh;commit;branch;diff;
StartupNotify=true
StartupWMClass=git-out
EOF
```

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl+O` / `Ctrl+T` | Open repository / open in new tab |
| `Ctrl+W` | Close current repository tab |
| `Ctrl+Tab` | Switch to next repository tab |
| `Ctrl+R` / `F5` | Refresh `git` snapshot & `gh` data |
| `Ctrl+1` .. `Ctrl+4` | Switch between **Graph**, **PRs**, **Actions**, **Issues** |
| `Ctrl+\`` | Toggle **Command Log** drawer |

---

## License

[MIT](LICENSE)
