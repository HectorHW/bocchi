use std::{
    collections::HashSet,
    marker::PhantomData,
    sync::MutexGuard,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use humantime::format_duration;
use itertools::Itertools;
use ringbuffer::{RingBuffer, RingBufferExt};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame, Terminal,
};

use crate::{
    configuration::FuzzConfig,
    execution::{ExecResult, RunTrace},
    state::{Library, State, AM},
};

pub struct TerminalUi<B: Backend + std::io::Write> {
    library: AM<Library>,
    state: AM<State>,
    terminal: Option<Terminal<B>>,
    config: &'static FuzzConfig,
}

impl TerminalUi<CrosstermBackend<std::io::Stdout>> {
    pub fn new(
        library: AM<Library>,
        state: AM<State>,
        config: &'static FuzzConfig,
    ) -> Result<Self, anyhow::Error> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(TerminalUi {
            library,
            state,
            terminal: Some(terminal),
            config,
        })
    }
}

struct TerminalInstance<'m, B: Backend + std::io::Write> {
    pub library: MutexGuard<'m, Library>,
    pub state: MutexGuard<'m, State>,
    pub config: &'static FuzzConfig,
    pub backend: PhantomData<B>,
}

impl<B: Backend + std::io::Write> TerminalUi<B> {
    pub fn tick(&mut self) -> Result<(), anyhow::Error> {
        let mut terminal = self.terminal.take().unwrap();

        terminal.draw(|frame| {
            let size: tui::layout::Rect = frame.size();

            let library = self.library.lock().unwrap();

            let state = self.state.lock().unwrap();

            let mut instance = TerminalInstance {
                library,
                state,
                config: self.config,
                backend: PhantomData {},
            };

            instance.draw_all(frame, size);
        })?;

        let _nothing = self.terminal.insert(terminal);
        Ok(())
    }
}

impl<'m, B: Backend + std::io::Write> TerminalInstance<'m, B> {
    fn draw_all(&mut self, frame: &mut Frame<B>, mut target: Rect) {
        self.draw_outer_frame(frame, target);

        target = target.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
            .split(target);

        self.write_left_panel(frame, layout[0]);

        self.write_right_panel(frame, layout[1]);

        //frame.render_widget(block, self.size);
    }

    fn draw_outer_frame(&mut self, frame: &mut Frame<B>, target: Rect) {
        let title = match &self.config.input {
            crate::configuration::InputOptions::Grammar { grammar } => {
                format!(
                    "bocchifuzz running {} with grammar {}",
                    self.config.binary.path, grammar
                )
            }
            crate::configuration::InputOptions::Seeds { seeds } => {
                format!(
                    "bocchifuzz running {} with seeds read from {}",
                    self.config.binary.path, seeds
                )
            }
        };

        let block = Block::default().title(title).borders(Borders::ALL);
        frame.render_widget(block, target);
    }

    fn extract_run_stats(&mut self) -> Vec<(String, String)> {
        vec![
            ("total".to_string(), self.state.tested_samples.to_string()),
            (
                "  - zero-exit".to_string(),
                self.state.total_working.to_string(),
            ),
            (
                "  - nonzero".to_string(),
                self.state.total_nonzero.to_string(),
            ),
            (
                "  - crashes".to_string(),
                self.state.total_crashes.to_string(),
            ),
            ("execution speed".to_string(), self.get_execution_speed()),
            (
                "size improvements".to_string(),
                self.state.improvements.to_string(),
            ),
        ]
    }

    fn extract_unique_stats(&mut self) -> Vec<(String, String)> {
        vec![
            ("unique paths".to_string(), self.library.len().to_string()),
            (
                "unique exit codes".to_string(),
                self.library
                    .iter()
                    .filter_map(|(trace, _sample)| {
                        if let RunTrace {
                            result: ExecResult::Code(code),
                            ..
                        } = trace
                        {
                            Some(code)
                        } else {
                            None
                        }
                    })
                    .collect::<HashSet<_>>()
                    .len()
                    .to_string(),
            ),
            (
                "unique crashes".to_string(),
                self.library
                    .iter()
                    .map(|p| p.0)
                    .filter(|run| matches!(run.result, ExecResult::Signal))
                    .count()
                    .to_string(),
            ),
        ]
    }

    fn format_duration(duration: Duration) -> String {
        format_duration(Duration::from_secs(duration.as_secs())).to_string()
    }

    fn get_run_duration(&self) -> String {
        let duration = Instant::now() - self.state.start_time;
        Self::format_duration(duration)
    }

    fn na_duration(point_in_the_past: Option<Instant>) -> String {
        point_in_the_past
            .map(|t| Self::format_duration(Instant::now() - t))
            .unwrap_or_else(|| "n/a".to_string())
    }

    fn extract_time_stats(&mut self) -> Vec<(String, String)> {
        vec![
            ("run duration".to_string(), self.get_run_duration()),
            (
                "last new path".to_string(),
                Self::na_duration(self.state.last_new_path),
            ),
            (
                "last new crash".to_string(),
                Self::na_duration(self.state.last_unique_crash),
            ),
        ]
    }

    fn get_execution_speed(&mut self) -> String {
        let now = Instant::now();

        self.state
            .executions
            .front()
            .map(|&time| {
                let items = self.state.executions.len() as f64;

                let duration = (now - time).as_secs_f64();

                items / duration
            })
            .map(|execs| format!("{:.1}/s", execs))
            .unwrap_or_else(|| "n/a".to_string())
    }

    fn write_stats(frame: &mut Frame<B>, target: Rect, stats: Vec<(String, String)>) {
        let rows = stats
            .into_iter()
            .map(|(k, v)| Row::new(vec![Cell::from(k), Cell::from(v)]).height(1));

        let table = Table::new(rows)
            .block(Block::default().borders(Borders::NONE))
            .widths(&[Constraint::Percentage(40), Constraint::Percentage(60)]);

        frame.render_widget(table, target);
    }

    fn write_list(frame: &mut Frame<B>, target: Rect, stats: Vec<String>) {
        let rows = stats
            .into_iter()
            .map(|item| Row::new(vec![Cell::from(item)]).height(1));

        let table = Table::new(rows)
            .block(Block::default().borders(Borders::NONE))
            .widths(&[Constraint::Percentage(100)]);

        frame.render_widget(table, target);
    }

    fn write_stats_in_frame(
        frame: &mut Frame<B>,
        mut target: Rect,
        stats: Vec<(String, String)>,
        title: &str,
    ) {
        let block = Block::default().title(title).borders(Borders::ALL);
        frame.render_widget(block, target);

        target = target.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        Self::write_stats(frame, target, stats)
    }

    fn write_list_in_frame(frame: &mut Frame<B>, mut target: Rect, list: Vec<String>, title: &str) {
        let block = Block::default().title(title).borders(Borders::ALL);
        frame.render_widget(block, target);

        target = target.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        Self::write_list(frame, target, list)
    }

    fn write_left_panel(&mut self, frame: &mut Frame<B>, target: Rect) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(30),
                    Constraint::Percentage(45),
                    Constraint::Percentage(25),
                ]
                .as_ref(),
            )
            .split(target);

        let time_stats = self.extract_time_stats();

        Self::write_stats_in_frame(frame, layout[0], time_stats, "time stats");

        let run_stats = self.extract_run_stats();

        Self::write_stats_in_frame(frame, layout[1], run_stats, "runs");

        let unique_stats = self.extract_unique_stats();

        Self::write_stats_in_frame(frame, layout[2], unique_stats, "uniques");
    }

    fn format_log(&self, space: Rect) -> Vec<String> {
        let log = crate::log::pull_messages(space.height as usize)
            .into_iter()
            .join("\n");

        textwrap::wrap(&log, space.width as usize)
            .iter()
            .take(space.height as usize)
            .map(|line| line.to_string())
            .collect_vec()
    }

    fn write_right_panel(&mut self, frame: &mut Frame<B>, target: Rect) {
        Self::write_list_in_frame(frame, target, self.format_log(target), "messages")
    }
}

impl<B: Backend + std::io::Write> Drop for TerminalUi<B> {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(
            self.terminal.as_mut().unwrap().backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
        self.terminal.as_mut().unwrap().show_cursor().unwrap();
    }
}

pub fn serve_ui(
    library: AM<Library>,
    state: AM<State>,
    config: &'static FuzzConfig,
) -> Result<(), anyhow::Error> {
    let mut ui = TerminalUi::new(library, state, config)?;

    const FRAME_RATE: u32 = 30;

    loop {
        ui.tick()?;

        if !event::poll(Duration::from_secs_f64(1.0 / (FRAME_RATE as f64)))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char('q') = key.code {
                return Ok(());
            }
        }
    }
}
