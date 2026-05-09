# Manual Reviewer — Zed CLI

\[[English](README.md)\] \[**中文**\]

> **厌倦了让 LLM 审查 LLM 写的代码？** 使用 Manual Reviewer 让你像一个真正的甲方一样使唤 AI。

## 安装

### macOS / Linux

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/jel1yspot/manual-reviewer-zed/main/scripts/install.sh)
```

### 从源码安装

```bash
cargo install --git https://github.com/jel1yspot/manual-reviewer-zed.git manual-reviewer-cli
mreview config-zed
```

## 手动接入 Zed（不用 `config-zed` 时的备选方案）

如果你想自己接（比如换个键位、或者只在某个项目下生效），模板在 `manual-reviewer-zed/.zed/tasks.json` 与 `manual-reviewer-zed/.zed/keymap.json`。

默认键位：

| 键位                              | task                                | 效果                                                                                                                              |
| --------------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `⇧-⌘-'` / `Ctrl-Shift-'`    | Manual Reviewer: Add for selection | 抓取当前选区（无选区时记为整文件 / 项目级）并在 Zed 终端打开多行评论编辑器                                                          |
| `⇧-⌘-I` / `Ctrl-Shift-I`    | Manual Reviewer: Export prompt     | 渲染 `.mreview/PROMPT.md`、复制到剪贴板、自动打开                                                                                |

其它任务（`Ctrl-Shift-R` 调出 `task: spawn` 查看）：**Manual Reviewer: List** 列出全部条目；**Manual Reviewer: Clear (archive)** 先归档再清空当前会话。

## 使用

在 Zed 里打开项目，选中一段代码，按 `⇧-⌘-'`（mac）或 `Ctrl-Shift-'`（linux）。Zed 的终端面板弹出，进入编辑器：

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

写下 comment，使用 **Enter** 提交，**Shift+Enter**（或 **Ctrl-J**）插入换行，**Esc** 取消。最后按 `⇧-⌘-I` / `Ctrl-Shift-I` 写入 `.mreview/PROMPT.md`、复制到剪贴板并用编辑器打开。

## CLI 命令

```
mreview add --from-zed-task                         # Zed task 用的入口
mreview add path/to/file:8:1-12:23 -m "your note"   # 手动入口；省略 -m 时弹编辑器
mreview list [--json]
mreview remove <SPEC>                               # SPEC: id | 1 | 1,3,5 | 2-4 | 1,3-5,8
mreview clear [--archive]
mreview export [--out PATH] [--format markdown|json] [--no-copy] [--no-open] [--no-write]
mreview config-zed [--dry-run]                      # 把 tasks/keymap 合并到 ~/.config/zed
```

默认读取 `$MREVIEW_ZED_WORKTREE_ROOT` 作为工作区目录，使用 `--workspace <PATH>`（或环境变量 `MREVIEW_WORKSPACE_ROOT`）覆盖工作区变量，变量不存在则会从 `$PWD` 向上查找 `.git` 目录。

## 开发

```bash
cargo test --workspace
cargo install --path crates/manual-reviewer-cli
```

## License

MIT
