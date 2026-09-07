//! Fullscreen agent picker: choose which ACP agent a new or loaded session
//! talks to. The caller builds the choice list — built-ins, platform routes,
//! and registry agents — and names the currently active source; this module
//! only renders the list and confirms a selection.

use std::io::Stdout;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{Event as CtEvent, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use ratatui::Terminal;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use tokio_util::sync::CancellationToken;

use crate::config::SelectedAgent;
use crate::ink::InkStyle;
use crate::palette::TerminalTheme;
use crate::term::TrackedBackend;

/// One selectable agent row.
#[derive(Debug, Clone)]
pub struct AgentChoice {
    pub source_id: String,
    pub label: String,
    /// Status summary shown after the label, e.g. `4 models` or
    /// `off — pick to enable`.
    pub status: String,
    /// Second line: launch evidence (e.g. `npx -y @google/gemini-cli --acp`).
    pub detail: String,
    /// The concrete ACP launch this choice starts.
    pub agent: SelectedAgent,
}

#[derive(Debug, Clone)]
pub enum AgentPickerOutcome {
    Selected(Box<AgentChoice>),
    Cancelled,
}

struct AgentPickerState {
    choices: Vec<AgentChoice>,
    selected: usize,
    current_source: Option<String>,
}

impl AgentPickerState {
    fn new(choices: Vec<AgentChoice>, current_source: Option<String>) -> Self {
        let selected = current_source
            .as_deref()
            .and_then(|source| choices.iter().position(|choice| choice.source_id == source))
            .unwrap_or(0);
        Self {
            choices,
            selected,
            current_source,
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.choices.is_empty() {
            return;
        }
        let len = self.choices.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
    }
}

/// Run the picker until the user selects an agent or cancels.
pub async fn run_agent_picker(
    terminal: &mut Terminal<TrackedBackend<Stdout>>,
    choices: Vec<AgentChoice>,
    current_source: Option<&str>,
    theme: TerminalTheme,
    termination: CancellationToken,
) -> Result<AgentPickerOutcome> {
    let mut state = AgentPickerState::new(choices, current_source.map(str::to_string));

    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(100));

    terminal.draw(|f| draw_agent_picker(f, &state, theme))?;

    loop {
        tokio::select! {
            biased;
            _ = termination.cancelled() => return Ok(AgentPickerOutcome::Cancelled),
            maybe_ev = events.next() => {
                let Some(ev) = maybe_ev else {
                    return Ok(AgentPickerOutcome::Cancelled);
                };
                let ev = ev.context("crossterm event stream")?;
                if let Some(outcome) = handle_agent_picker_event(&mut state, ev) {
                    return Ok(outcome);
                }
            }
            _ = tick.tick() => {}
        }
        terminal.draw(|f| draw_agent_picker(f, &state, theme))?;
    }
}

fn handle_agent_picker_event(
    state: &mut AgentPickerState,
    ev: CtEvent,
) -> Option<AgentPickerOutcome> {
    let CtEvent::Key(key) = ev else {
        return None;
    };
    if key.kind != KeyEventKind::Press {
        return None;
    }
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyCode::Esc) => {
            Some(AgentPickerOutcome::Cancelled)
        }
        (_, KeyCode::Up) => {
            state.move_selection(-1);
            None
        }
        (_, KeyCode::Down) => {
            state.move_selection(1);
            None
        }
        (_, KeyCode::Enter) => state
            .choices
            .get(state.selected)
            .cloned()
            .map(Box::new)
            .map(AgentPickerOutcome::Selected),
        _ => None,
    }
}

fn draw_agent_picker(frame: &mut ratatui::Frame, state: &AgentPickerState, theme: TerminalTheme) {
    let area = frame.area();
    if area.width < 60 || area.height < 8 {
        return;
    }
    let row_count = state.choices.len() as u16;
    let height = (row_count.saturating_mul(2).saturating_add(3)).clamp(8, 24);
    let rect = crate::term::centered_rect(area, 88, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .title(" Select agent ")
        .borders(Borders::ALL)
        .style(Style::default().ink(theme.text));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let rows_available = (inner.height.saturating_sub(1) / 2) as usize;
    let lines = agent_choice_lines(state, rows_available, rows[0].width as usize, theme);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), rows[0]);
    frame.render_widget(
        Paragraph::new("↑/↓ select · Enter confirm · Esc cancel")
            .style(Style::default().ink(theme.muted)),
        rows[1],
    );
}

fn agent_choice_lines(
    state: &AgentPickerState,
    rows_available: usize,
    width: usize,
    theme: TerminalTheme,
) -> Vec<Line<'static>> {
    // Two lines per agent: the selected row and its detail line stay visible
    // together, so the window follows the selection like the settings panel.
    let rows_available = rows_available.max(1);
    let start = state
        .selected
        .saturating_sub(rows_available.saturating_sub(1) / 2);
    let mut lines = Vec::new();
    for (index, choice) in state.choices.iter().enumerate().skip(start) {
        let selected = index == state.selected;
        let marker = if selected { "›" } else { " " };
        let current = state
            .current_source
            .as_deref()
            .is_some_and(|source| source == choice.source_id);
        let suffix = if current { " · current" } else { "" };
        let text = format!("{marker} {:<24} · {}{suffix}", choice.label, choice.status);
        lines.push(Line::from(Span::styled(
            fit_width(text, width),
            if selected {
                Style::default()
                    .ink(theme.selection_fg)
                    .ink_bg(theme.selection_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().ink(theme.text)
            },
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "      {}",
                fit_width(&choice.detail, width.saturating_sub(6))
            ),
            Style::default().ink(theme.muted),
        )));
    }
    lines
}

fn fit_width(text: impl AsRef<str>, width: usize) -> String {
    let text = text.as_ref();
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars()
        .take(width.saturating_sub(1))
        .collect::<String>()
        + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventState};
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn choice(id: &str, label: &str) -> AgentChoice {
        AgentChoice {
            source_id: id.to_string(),
            label: label.to_string(),
            status: "2 models".to_string(),
            detail: format!("npx -y {id}"),
            agent: SelectedAgent {
                source_id: id.to_string(),
                program: PathBuf::from("npx"),
                args: vec![id.to_string()],
                env: HashMap::new(),
            },
        }
    }

    fn key(code: KeyCode) -> CtEvent {
        CtEvent::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn picker_preselects_the_current_source() {
        let choices = vec![choice("gemini", "Gemini"), choice("codex-acp", "Codex")];
        let state = AgentPickerState::new(choices.clone(), Some("codex-acp".to_string()));
        assert_eq!(state.selected, 1);

        let state = AgentPickerState::new(choices, None);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn picker_missing_current_source_falls_back_to_the_first_choice() {
        let state = AgentPickerState::new(
            vec![choice("gemini", "Gemini")],
            Some("unknown".to_string()),
        );
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn picker_move_selection_wraps() {
        let mut state = AgentPickerState::new(
            vec![choice("a", "A"), choice("b", "B"), choice("c", "C")],
            None,
        );
        state.move_selection(-1);
        assert_eq!(state.selected, 2);
        state.move_selection(1);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn picker_enter_returns_the_focused_choice() {
        let mut state = AgentPickerState::new(
            vec![choice("gemini", "Gemini"), choice("opencode", "OpenCode")],
            Some("gemini".to_string()),
        );
        state.move_selection(1);
        let outcome =
            handle_agent_picker_event(&mut state, key(KeyCode::Enter)).expect("enter selects");
        match outcome {
            AgentPickerOutcome::Selected(choice) => assert_eq!(choice.source_id, "opencode"),
            AgentPickerOutcome::Cancelled => panic!("enter must select, not cancel"),
        }
    }

    #[test]
    fn picker_escape_and_ctrl_c_cancel() {
        let mut state = AgentPickerState::new(vec![choice("gemini", "Gemini")], None);
        assert!(matches!(
            handle_agent_picker_event(&mut state, key(KeyCode::Esc)),
            Some(AgentPickerOutcome::Cancelled)
        ));
        let mut state = AgentPickerState::new(vec![choice("gemini", "Gemini")], None);
        let ctrl_c = CtEvent::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        });
        assert!(matches!(
            handle_agent_picker_event(&mut state, ctrl_c),
            Some(AgentPickerOutcome::Cancelled)
        ));
    }

    #[test]
    fn picker_ignores_non_press_events() {
        let mut state = AgentPickerState::new(vec![choice("gemini", "Gemini")], None);
        let release = CtEvent::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        });
        assert!(handle_agent_picker_event(&mut state, release).is_none());
    }

    #[test]
    fn picker_draws_without_panicking_on_tiny_and_empty_screens() {
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let state = AgentPickerState::new(vec![choice("gemini", "Gemini")], None);
        terminal
            .draw(|f| draw_agent_picker(f, &state, TerminalTheme::current()))
            .expect("draw");

        let backend = ratatui::backend::TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|f| draw_agent_picker(f, &state, TerminalTheme::current()))
            .expect("small screen draw");
    }
}
