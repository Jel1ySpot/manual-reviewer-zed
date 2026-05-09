use std::borrow::Cow;
use std::io::{self, Write};

use anyhow::{anyhow, Context, Result};
use reedline::{
    default_emacs_keybindings, EditCommand, Emacs, KeyCode, KeyModifiers, Prompt,
    PromptEditMode, PromptHistorySearch, PromptViMode, Reedline, ReedlineEvent, Signal,
};

/// Read a multi-line comment from the terminal.
///
/// Backed by [`reedline`] (Nushell's line editor), which gives us proper cursor
/// movement, word jumps, history-free single-prompt mode, and clean redraw —
/// everything our hand-rolled crossterm editor used to fake by hand.
///
/// Keys:
/// - **Enter** submits.
/// - **Shift+Enter** inserts a newline (requires kitty keyboard protocol).
/// - **Ctrl-J** also inserts a newline (always works — fallback).
/// - **Esc** or **Ctrl-C** cancels.
pub fn read_multiline_comment(intro: &str) -> Result<String> {
    {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        writeln!(out, "{}", intro)?;
        writeln!(
            out,
            "(Enter = submit · Shift+Enter or Ctrl-J = newline · Esc = cancel)"
        )?;
        out.flush()?;
    }

    let mut bindings = default_emacs_keybindings();
    bindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );
    bindings.add_binding(
        KeyModifiers::CONTROL,
        KeyCode::Char('j'),
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );
    bindings.add_binding(KeyModifiers::NONE, KeyCode::Esc, ReedlineEvent::CtrlC);

    let edit_mode = Box::new(Emacs::new(bindings));
    let mut editor = Reedline::create()
        .with_edit_mode(edit_mode)
        .use_kitty_keyboard_enhancement(true);

    let prompt = MultilinePrompt;
    match editor.read_line(&prompt) {
        Ok(Signal::Success(text)) => {
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                Err(anyhow!("Empty comment — entry not added."))
            } else {
                Ok(trimmed)
            }
        }
        Ok(Signal::CtrlC) | Ok(Signal::CtrlD) => Err(anyhow!("Cancelled.")),
        Ok(other) => Err(anyhow!("unexpected reedline signal: {:?}", other)),
        Err(e) => Err(anyhow::Error::new(e)).context("read multi-line input"),
    }
}

/// Print a fenced preview of the captured selection. `start_column` is the
/// 1-based column where the selection starts on its first line — used to
/// re-indent the first snippet line so it visually lines up with the source.
pub fn print_selection_preview(
    file: &str,
    range_label: &str,
    language: &str,
    snippet: &str,
    start_column: u32,
) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let lang = if language.is_empty() {
        String::new()
    } else {
        format!(" · {}", language)
    };
    let header = format!("{}<{}>{}", file, range_label, lang);
    let total_width: usize = 72;
    let used = header.chars().count() + 4;
    let pad_chars = total_width.saturating_sub(used);
    let pad: String = "─".repeat(pad_chars);
    let full_bar: String = "─".repeat(total_width);
    writeln!(out, "┌─ {} {}", header, pad)?;

    let first_indent = " ".repeat((start_column.saturating_sub(1)) as usize);
    let lines: Vec<&str> = snippet.lines().collect();
    for (i, line) in lines.iter().take(20).enumerate() {
        if i == 0 && !first_indent.is_empty() && lines.len() > 1 {
            writeln!(out, "│ {}{}", first_indent, line)?;
        } else {
            writeln!(out, "│ {}", line)?;
        }
    }
    if lines.len() > 20 {
        writeln!(out, "│ … ({} more lines)", lines.len() - 20)?;
    }
    writeln!(out, "└{}", full_bar)?;
    out.flush()?;
    Ok(())
}

struct MultilinePrompt;

impl Prompt for MultilinePrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, mode: PromptEditMode) -> Cow<'_, str> {
        match mode {
            PromptEditMode::Default | PromptEditMode::Emacs => Cow::Borrowed(""),
            PromptEditMode::Vi(PromptViMode::Normal) => Cow::Borrowed("[N] "),
            PromptEditMode::Vi(PromptViMode::Insert) => Cow::Borrowed(""),
            PromptEditMode::Custom(_) => Cow::Borrowed(""),
        }
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("  ")
    }
    fn render_prompt_history_search_indicator(
        &self,
        _: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
}
