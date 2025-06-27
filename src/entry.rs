use anyhow::{Result, anyhow};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio_stream::StreamExt;

use crossterm::event::{Event, EventStream, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Style, Styled, Stylize},
    text::Line,
    widgets::{Block, HighlightSpacing, Row, StatefulWidget, Table, TableState, Widget},
};
use sysfs_gpio::{Direction, Pin};

const PIN_COUNT: usize = 256;

pub async fn main() -> Result<()> {
    color_eyre::install().map_err(|err| anyhow!(err))?;
    let terminal = ratatui::init();
    App::default().run(terminal).await?;
    ratatui::try_restore()?;
    Ok(())
}

#[derive(Debug, Default)]
struct App {
    should_quit: bool,
    widget: ListWidget,
}

impl App {
    pub async fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        const FRAMES_PER_SECOND: f32 = 60.;
        self.widget.run();
        let period = Duration::from_secs_f32(1. / FRAMES_PER_SECOND);
        let mut interval = tokio::time::interval(period);
        let mut events = EventStream::new();
        while !self.should_quit {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.widget.fetch_gpios().await {
                        eprintln!("Error fetching GPIOs: {}", e);
                    }
                    terminal.draw(|frame| self.render(frame))?;
                },
                Some(Ok(event)) = events.next() => self.handle_event(&event),
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
        let [title_area, body_area] = vertical.areas(frame.area());
        let title = Line::from("GPIO line locator").centered().bold();
        frame.render_widget(title, title_area);
        frame.render_widget(self.widget.clone(), body_area);
    }

    fn handle_event(&mut self, event: &Event) {
        if let Some(key) = event.as_key_press_event() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
                KeyCode::Char('j') | KeyCode::Down => self.widget.scroll_down(),
                KeyCode::Char('k') | KeyCode::Up => self.widget.scroll_up(),
                KeyCode::Char('r') => self.widget.refresh_change_state(),
                KeyCode::Char('b') => self.widget.block_changed_pins(),
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ListWidget {
    state: Arc<Mutex<State>>,
    table_state: Arc<Mutex<TableState>>,
}

#[derive(Debug, Clone)]
struct State {
    gpios: Vec<Option<bool>>,
    prev_gpios: Vec<Option<bool>>, // 存储上一次的电平状态
    gpios_has_changed: Vec<bool>,  // 存储每个GPIO是否有变化
    pins: Vec<Option<Pin>>,
    last_update: std::time::Instant,
}

impl Default for State {
    fn default() -> Self {
        for i in 0..PIN_COUNT {
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("echo {} > /sys/class/gpio/export", i))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }

        Self {
            gpios: vec![None; PIN_COUNT],
            prev_gpios: vec![None; PIN_COUNT], // 初始化为None
            gpios_has_changed: vec![false; PIN_COUNT],
            pins: (0..PIN_COUNT)
                .map(|i| {
                    let pin = Pin::new(i as u64);
                    if pin.set_direction(Direction::In).is_err() {
                        None
                    } else if pin.is_exported() {
                        Some(pin)
                    } else if pin.export().is_err() {
                        None
                    } else {
                        Some(pin)
                    }
                })
                .collect::<Vec<_>>(),
            last_update: std::time::Instant::now(),
        }
    }
}

impl ListWidget {
    fn run(&self) {
        let this = self.clone(); // clone the widget to pass to the background task
        tokio::spawn(async move { this.init_gpios().await });
    }

    async fn init_gpios(&self) -> Result<()> {
        if let Ok(mut state) = self.state.lock() {
            for i in 0..PIN_COUNT {
                let pin = &state.pins[i];
                let current_value = pin
                    .map(|pin| pin.get_value().map(|val| val != 0).ok())
                    .flatten();

                state.prev_gpios[i] = current_value;
                state.gpios[i] = current_value;
            }

            state.last_update = std::time::Instant::now();
        }
        Ok(())
    }

    async fn fetch_gpios(&self) -> Result<()> {
        if let Ok(mut state) = self.state.lock() {
            for i in 0..PIN_COUNT {
                let pin = &state.pins[i];
                let current_value = pin
                    .map(|pin| pin.get_value().map(|val| val != 0).ok())
                    .flatten();

                // 获取上一次的值
                let prev_value = state.prev_gpios[i];

                // 检查电平是否发生变化
                let changed = match (prev_value, current_value) {
                    (Some(prev), Some(current)) => prev != current,
                    (None, Some(_)) => true, // 之前没有值，现在有值
                    (Some(_), None) => true, // 之前有值，现在没有值
                    (None, None) => false,   // 一直都没有值
                };

                state.prev_gpios[i] = current_value;
                state.gpios[i] = current_value;
                state.gpios_has_changed[i] = changed || state.gpios_has_changed[i];
            }

            state.last_update = std::time::Instant::now();
        }
        Ok(())
    }

    fn refresh_change_state(&self) {
        if let Ok(mut state) = self.state.lock() {
            // 重置变化状态
            state.gpios_has_changed.fill(false);
        }
    }

    fn block_changed_pins(&self) {
        if let Ok(mut state) = self.state.lock() {
            let changed_indices: Vec<usize> = state
                .gpios_has_changed
                .iter()
                .enumerate()
                .filter_map(|(i, changed)| if *changed { Some(i) } else { None })
                .collect();
            for idx in changed_indices {
                if let Some(pin) = state.pins.get_mut(idx) {
                    *pin = None;
                }
            }
        }
    }

    fn scroll_down(&self) {
        let _ = self
            .table_state
            .lock()
            .map(|mut table| table.scroll_down_by(1));
    }

    fn scroll_up(&self) {
        let _ = self
            .table_state
            .lock()
            .map(|mut table| table.scroll_up_by(1));
    }
}

impl Widget for ListWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let state = self.state.lock().unwrap().clone();
        let block = Block::bordered()
            .title(format!(
                "Delay {}μs",
                state.last_update.elapsed().as_micros()
            ))
            .title_bottom("q to quit, r to refresh, j/k to scroll, b to block changed pins");

        // 8x8 GPIO matrix table
        let rows = (0..PIN_COUNT / 8).map(|row| {
            let cells = (0..8).map(|col| {
                let idx = row * 8 + col;
                let val = state.gpios.get(idx).copied().unwrap_or(None);
                let changed = state.gpios_has_changed.get(idx).copied().unwrap_or(false);
                let (text, style) = match (val, changed) {
                    // 有变化的电平（无论高低）都显示黄色背景
                    (Some(_), true) => (format!("Pin{}", idx), Style::new().on_yellow()),
                    // 稳定的高电平 - 绿色背景
                    (Some(true), false) => (format!("Pin{}", idx), Style::new().on_green()),
                    // 稳定的低电平 - 蓝色背景
                    (Some(false), false) => (format!("Pin{}", idx), Style::new().on_blue()),
                    // 无法读取的状态 - 灰色背景
                    (None, _) => (format!("Pin{}", idx), Style::new().on_dark_gray()),
                };
                text.set_style(style)
            });
            Row::new(cells)
        });

        let widths = [Constraint::Length(12); 8];
        let table = Table::new(rows, widths)
            .block(block)
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(">>");

        StatefulWidget::render(table, area, buf, &mut self.table_state.lock().unwrap());
    }
}
