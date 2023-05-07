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
use itertools::Itertools;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame, Terminal,
};

use crate::{
    execution::{ExecResult, RunTrace},
    state::{Library, State, AM},
};

pub struct TerminalUi<B: Backend + std::io::Write> {
    library: AM<Library>,
    state: AM<State>,
    terminal: Option<Terminal<B>>,
}

impl TerminalUi<CrosstermBackend<std::io::Stdout>> {
    pub fn new(library: AM<Library>, state: AM<State>) -> Result<Self, anyhow::Error> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(TerminalUi {
            library,
            state,
            terminal: Some(terminal),
        })
    }
}

struct TerminalInstance<'m, B: Backend + std::io::Write> {
    pub library: MutexGuard<'m, Library>,
    pub state: MutexGuard<'m, State>,
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
        let block = Block::default().title("bocchifuzz").borders(Borders::ALL);
        frame.render_widget(block, target);
    }

    fn extract_run_stats(&mut self) -> Vec<(String, String)> {
        vec![
            (
                "total runs".to_string(),
                self.state.tested_samples.to_string(),
            ),
            (
                "size improvements".to_string(),
                self.state.improvements.to_string(),
            ),
            (
                "total zero-exit".to_string(),
                self.state.total_working.to_string(),
            ),
            (
                "total nonzero".to_string(),
                self.state.total_nonzero.to_string(),
            ),
            (
                "total crashes".to_string(),
                self.state.total_crashes.to_string(),
            ),
        ]
    }

    fn extract_unique_stats(&mut self) -> Vec<(String, String)> {
        vec![
            ("unique paths".to_string(), self.library.len().to_string()),
            (
                "unique exit codes".to_string(),
                self.library
                    .keys()
                    .iter()
                    .filter_map(|run| {
                        if let RunTrace {
                            result: ExecResult::Code(code),
                            ..
                        } = run
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
                    .keys()
                    .iter()
                    .filter(|run| matches!(run.result, ExecResult::Signal))
                    .count()
                    .to_string(),
            ),
        ]
    }

    fn get_run_duration(&self) -> String {
        let duration = Instant::now() - self.state.start_time;

        let duration = chrono::Duration::from_std(duration).unwrap();

        duration.to_string()
    }

    fn extract_time_stats(&mut self) -> Vec<(String, String)> {
        vec![("run duration".to_string(), self.get_run_duration())]
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
                    Constraint::Percentage(20),
                    Constraint::Percentage(50),
                    Constraint::Percentage(30),
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

    fn format_crashes(&mut self) -> Vec<String> {
        self.library
            .keys()
            .iter()
            .zip(self.library.values().iter())
            .filter(|(k, _v)| k.result == ExecResult::Signal)
            .filter_map(|(crash_trace, _sample)| {
                crate::sample_library::Library::get_detailed_trace(&*self.library, crash_trace)
                    .map(|detailed| detailed.iter().map(|n| format!("{n:x}")).join(" "))
            })
            .collect_vec()
    }

    fn write_right_panel(&mut self, frame: &mut Frame<B>, target: Rect) {
        Self::write_list_in_frame(frame, target, self.format_crashes(), "crashes")
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

pub fn serve_ui(library: AM<Library>, state: AM<State>) -> Result<(), anyhow::Error> {
    let mut ui = TerminalUi::new(library, state)?;

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
