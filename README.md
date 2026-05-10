# Manual Reviewer — Zed CLI

\[**English**\] \[[中文](README_zh.md)\]

> **Tired of letting LLMs review LLM-written code?** Use Manual Reviewer to boss AI around like a real client.

## Installation

### macOS / Linux

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/jel1yspot/manual-reviewer-zed/main/scripts/install.sh)
```

### From source

```bash
cargo install --git https://github.com/jel1yspot/manual-reviewer-zed.git manual-reviewer-cli
mreview config-zed
```

## Wire into Zed by hand (alternative to `config-zed`)

If you'd rather wire it up yourself (different keybinding, per-project only, etc.), the templates live at `manual-reviewer-zed/.zed/tasks.json` and `manual-reviewer-zed/.zed/keymap.json`.

Default keybindings:

| Keys                              | Task                               | What                                                                                                                              |
| --------------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `⇧-⌘-'` / `Ctrl-Shift-'`    | Manual Reviewer: Add for selection | Capture the current selection (or whole-file / project-level if no selection) and open the multi-line comment editor in Zed's terminal |
| `⇧-⌘-I` / `Ctrl-Shift-I`    | Manual Reviewer: Export prompt     | Render `.mreview/PROMPT.md`, copy to clipboard, open it                                                                          |

Other tasks (use `Ctrl-Shift-R` to surface `task: spawn`): **Manual Reviewer: List** prints all entries; **Manual Reviewer: Clear (archive)** archives then clears the current session.

## Usage

Open a project in Zed, select some code, press `⇧-⌘-'` (mac) or `Ctrl-Shift-'` (linux). Zed's terminal panel appears with the editor:

```
┌─ src/foo.rs<42:5-58:23> · rust ────────────
│     fn save_session(...) {
│         for _ in 0..3 { ... }
│     }
└────────────────────────────────────────────
Write your comment:
(Enter = submit · Shift+Enter or Ctrl-J = newline · Esc = cancel)
> _
```

Write your comment. **Enter** submits, **Shift+Enter** (or **Ctrl-J**) inserts a newline, **Esc** cancels. When done, press `⇧-⌘-I` / `Ctrl-Shift-I` to write `.mreview/PROMPT.md`, copy it to your clipboard, and open it in your editor.

## CLI commands

```
mreview add --from-env                         # Zed task entry point
mreview add path/to/file:8:1-12:23 -m "your note"   # manual; omit -m to open the editor
mreview list [--json]
mreview remove <SPEC>                               # SPEC: id | 1 | 1,3,5 | 2-4 | 1,3-5,8
mreview clear [--archive]
mreview export [--out PATH] [--format markdown|json] [--editor CMD] [--no-copy] [--no-open] [--no-write]
mreview config-zed [--dry-run]                      # merge tasks/keymap into ~/.config/zed
```

Workspace defaults to `$MREVIEW_WORKTREE_ROOT`. Use `--workspace <PATH>` (or env `MREVIEW_WORKSPACE_ROOT`) to override; if neither is set, the CLI walks up from `$PWD` looking for `.git`.

## Development

```bash
cargo test --workspace
cargo install --path crates/manual-reviewer-cli
```

## License

MIT
