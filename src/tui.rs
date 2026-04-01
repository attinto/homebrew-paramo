use crate::blocker::{self, StatusSnapshot};
use crate::config::{SiteMutation, SystemConfig};
use crate::doctor::{self, Diagnostic, DiagnosticLevel};
use crate::i18n::{I18n, Language};
use crate::ipc;
use crate::journal;
use crate::paths;
use crate::preferences::UserPreferences;
use anyhow::Result;
use chrono::Datelike;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

const ASCII_ART: &[&str] = &[
    r"                                   /\",
    r"                              /\  //\\",
    r"                       /\    //\\///\\\        /\",
    r"                      //\\  ///\////\\\\  /\  //\\",
    r"         /\          /  ^ \/^ ^/^  ^  ^ \/^ \/  ^ \",
    r"        / ^\    /\  / ^   /  ^/ ^ ^ ^   ^\ ^/  ^^  \",
    r"       /^   \  / ^\/ ^ ^   ^ / ^  ^    ^  \/ ^   ^  \       *",
    r"      /  ^ ^ \/^  ^\ ^ ^ ^   ^  ^   ^   ____  ^   ^  \     /|\",
    r"     / ^ ^  ^ \ ^  _\___________________|  |_____^ ^  \   /||o\",
    r"    / ^^  ^ ^ ^\  /______________________________\ ^ ^ \ /|o|||\",
    r"   /  ^  ^^ ^ ^  /________________________________\  ^  /|||||o|\",
    r"  /^ ^  ^ ^^  ^    ||___|___||||||||||||___|__|||      /||o||||||\",
    r" / ^   ^   ^    ^  ||___|___||||||||||||___|__|||          | |",
    r"/ ^ ^ ^  ^  ^  ^   ||||||||||||||||||||||||||||||oooooooooo| |ooooooo",
    r"ooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo",
];

const DISTRACTION_PHRASES: &[&str] = &[
    "Brillando",
    "Zumbando",
    "Revoloteando",
    "Saltando",
    "Flotando",
    "Destellando",
    "Chapoteando",
    "Sonriendo",
    "Resplandeciendo",
    "Canturreando",
    "Dançando",
    "Girando",
    "Burbujeando",
    "Crepitando",
    "Pintando",
    "Tejiendo",
    "Explorando",
    "Saltimbanqueando",
    "Chapurreando",
    "Lumineando",
    "Zarandeando",
    "Tintineando",
    "Rebotando",
    "Culebreando",
    "Espiraleando",
    "Saltarínando",
    "Voloteando",
    "Susurrando",
    "Chispeando",
    "Danilando",
    "Burbujeleteando",
    "Prismando",
    "Meneoando",
    "Brincolineando",
    "Planteando",
    "Revolineando",
    "Sonorineando",
    "Floreando",
    "Risueñeando",
    "Lumineleando",
    "Carcajeando",
    "Tintoreando",
    "Brillulineando",
    "Zumbileando",
    "Espumando",
    "Chapoteleando",
    "Vuelineando",
    "Trinando",
    "Susurrileando",
    "Ventileando",
];

const WAVE_FRAMES: &[&str] = &[
    "≈   ≈   ≈   ≈   ≈   ≈   ≈",
    " ≈   ≈   ≈   ≈   ≈   ≈   ",
    "  ≈   ≈   ≈   ≈   ≈   ≈  ",
    "   ≈   ≈   ≈   ≈   ≈   ≈ ",
    "  ≈   ≈   ≈   ≈   ≈   ≈  ",
    " ≈   ≈   ≈   ≈   ≈   ≈   ",
];

#[derive(Debug)]
enum UnblockFlow {
    Countdown { started: Instant, duration_secs: u64 },
    ReasonPrompt { value: String },
    FinalCountdown { started: Instant, reason: String },
}

#[derive(Debug)]
enum RemoveConfirmFlow {
    Typing { site: String, typed: String },
}

#[derive(Debug, Clone, Copy)]
enum TabId {
    Home,
    Sites,
    Schedule,
    Settings,
    Diagnostics,
    Wall,
    Exit,
}

impl TabId {
    fn all() -> [Self; 7] {
        [
            Self::Home,
            Self::Sites,
            Self::Schedule,
            Self::Settings,
            Self::Diagnostics,
            Self::Wall,
            Self::Exit,
        ]
    }

    fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::Sites => 1,
            Self::Schedule => 2,
            Self::Settings => 3,
            Self::Diagnostics => 4,
            Self::Wall => 5,
            Self::Exit => 6,
        }
    }

    fn from_index(index: usize) -> Self {
        Self::all()[index % Self::all().len()]
    }
}

#[derive(Debug, Clone)]
struct PromptState {
    title: String,
    value: String,
}

#[derive(Debug)]
struct Dashboard {
    config: SystemConfig,
    prefs: UserPreferences,
    i18n: I18n,
    status: StatusSnapshot,
    diagnostics: Vec<Diagnostic>,
    active_tab: TabId,
    sites_state: ListState,
    diagnostics_state: ListState,
    schedule_cursor: usize,
    flash_message: Option<String>,
    prompt: Option<PromptState>,
    unblock_flow: Option<UnblockFlow>,
    pending_unblock: Option<String>,
    wall_entries: Vec<journal::JournalEntry>,
    wall_state: ListState,
    remove_flow: Option<RemoveConfirmFlow>,
}

pub fn run(config: &mut SystemConfig, prefs: &mut UserPreferences) -> Result<()> {
    let mut dashboard = Dashboard::new(config.clone(), prefs.clone())?;
    let mut terminal = TerminalSession::new()?;

    loop {
        terminal.terminal.draw(|frame| dashboard.render(frame))?;
        dashboard.perform_pending_actions()?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if dashboard.handle_key(key)? {
                    break;
                }
            }
        }
    }

    *config = dashboard.config;
    *prefs = dashboard.prefs;
    Ok(())
}

impl Dashboard {
    fn new(config: SystemConfig, prefs: UserPreferences) -> Result<Self> {
        let i18n = I18n::new(prefs.language);
        let diagnostics = doctor::run(&config, i18n)?;
        let status = blocker::status_snapshot(&config)?;
        let mut sites_state = ListState::default();
        let mut diagnostics_state = ListState::default();

        if !config.sites.list.is_empty() {
            sites_state.select(Some(0));
        }
        if !diagnostics.is_empty() {
            diagnostics_state.select(Some(0));
        }

        Ok(Self {
            config,
            prefs,
            i18n,
            status,
            diagnostics,
            active_tab: TabId::Home,
            sites_state,
            diagnostics_state,
            schedule_cursor: 0,
            flash_message: None,
            prompt: None,
            unblock_flow: None,
            pending_unblock: None,
            wall_entries: journal::load().unwrap_or_default(),
            wall_state: ListState::default(),
            remove_flow: None,
        })
    }

    fn render(&mut self, frame: &mut Frame) {
        self.advance_unblock_flow();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(18),
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(3),
            ])
            .split(frame.area());

        self.render_header(frame, outer[0]);
        self.render_tabs(frame, outer[1]);
        self.render_body(frame, outer[2]);
        self.render_footer(frame, outer[3]);

        if let Some(prompt) = &self.prompt {
            self.render_prompt(frame, prompt);
        }

        self.render_unblock_flow(frame);
        self.render_remove_flow(frame);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(paths::APP_DISPLAY_NAME);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(74), Constraint::Percentage(26)])
            .split(inner);

        let art = ASCII_ART
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    *line,
                    Style::default()
                        .fg(Color::Rgb(177, 214, 166))
                        .add_modifier(Modifier::BOLD),
                ))
            })
            .collect::<Vec<_>>();

        let subtitle = match self.i18n.language() {
            Language::Es => "Aíslate para construir",
            Language::En => "Isolate to build",
        };

        let side_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                paths::APP_DISPLAY_NAME,
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                subtitle,
                Style::default().fg(Color::Rgb(231, 223, 201)),
            )),
            Line::from(""),
            Line::from(Span::styled(
                self.i18n.header_nav_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
            Line::from(Span::styled(
                self.i18n.header_confirm_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
            Line::from(Span::styled(
                self.i18n.header_quit_hint(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )),
        ];

        frame.render_widget(
            Paragraph::new(Text::from(art))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
            sections[0],
        );
        frame.render_widget(
            Paragraph::new(side_text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            sections[1],
        );
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let titles = vec![
            self.i18n.home_tab(),
            self.i18n.sites_tab(),
            self.i18n.schedule_tab(),
            self.i18n.settings_tab(),
            self.i18n.diagnostics_tab(),
            self.i18n.wall_tab(),
            self.i18n.exit_tab(),
        ]
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();

        let tabs = Tabs::new(titles)
            .select(self.active_tab.index())
            .style(Style::default().fg(Color::Rgb(190, 197, 208)))
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" ")
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(tabs, area);
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        match self.active_tab {
            TabId::Home => self.render_home(frame, area),
            TabId::Sites => self.render_sites(frame, area),
            TabId::Schedule => self.render_schedule(frame, area),
            TabId::Settings => self.render_settings(frame, area),
            TabId::Diagnostics => self.render_diagnostics(frame, area),
            TabId::Wall => self.render_wall(frame, area),
            TabId::Exit => self.render_exit(frame, area),
        }
    }

    fn render_home(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let state_label = if self.status.hosts_blocked {
            self.i18n.blocked_label()
        } else {
            self.i18n.unblocked_label()
        };
        let schedule_label = if self.status.schedule_active {
            self.i18n.schedule_active_label()
        } else {
            self.i18n.schedule_inactive_label()
        };

        let next_change = self
            .status
            .next_transition
            .map(|next| next.format("%H:%M").to_string())
            .unwrap_or_else(|| "--:--".to_string());

        let monk_line = if self.status.monk_mode {
            Line::from(Span::styled(
                format!("⛰  {} — {}", self.i18n.monk_mode_label(), self.i18n.monk_mode_active_label()),
                Style::default()
                    .fg(Color::Rgb(224, 102, 102))
                    .add_modifier(Modifier::BOLD),
            ))
        } else {
            Line::from("")
        };

        let home_lines = vec![
            Line::from(format!(
                "{} {}",
                self.i18n.weekday(self.status.now.weekday()),
                self.status.now.format("%H:%M")
            )),
            Line::from(""),
            Line::from(Span::styled(
                state_label,
                Style::default()
                    .fg(if self.status.hosts_blocked {
                        Color::Rgb(224, 102, 102)
                    } else {
                        Color::Rgb(123, 201, 111)
                    })
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(schedule_label),
            Line::from(format!(
                "{}: {}",
                self.i18n.configured_sites_label(),
                self.status.site_count
            )),
            Line::from(format!(
                "{}: {}",
                self.i18n.next_change_label(),
                next_change
            )),
            Line::from(""),
            monk_line,
        ];

        let status_panel = Paragraph::new(home_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match self.i18n.language() {
                        Language::Es => "Estado",
                        Language::En => "Status",
                    }),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(status_panel, columns[0]);

        let actions = vec![
            Line::from(self.i18n.home_action_block()),
            Line::from(self.i18n.home_action_unblock()),
            Line::from(self.i18n.home_action_refresh()),
            Line::from(self.i18n.home_action_exit()),
        ];

        let actions_panel = Paragraph::new(actions)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.home_actions()),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(actions_panel, columns[1]);
    }

    fn render_sites(&mut self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let items = if self.config.sites.list.is_empty() {
            vec![ListItem::new(self.i18n.site_empty())]
        } else {
            self.config
                .sites
                .list
                .iter()
                .map(|site| ListItem::new(site.clone()))
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.sites_tab()),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");

        frame.render_stateful_widget(list, columns[0], &mut self.sites_state);

        let selected_site = self
            .sites_state
            .selected()
            .and_then(|index| self.config.sites.list.get(index))
            .cloned()
            .unwrap_or_else(|| "-".to_string());

        let side = Paragraph::new(vec![
            Line::from(format!(
                "{}: {}",
                self.i18n.configured_sites_label(),
                self.config.sites.list.len()
            )),
            Line::from(format!(
                "{}: {}",
                match self.i18n.language() {
                    Language::Es => "Seleccionado",
                    Language::En => "Selected",
                },
                selected_site
            )),
            Line::from(""),
            Line::from(match self.i18n.language() {
                Language::Es => "a  Añadir sitio",
                Language::En => "a  Add site",
            }),
            Line::from(match self.i18n.language() {
                Language::Es => "d  Eliminar sitio",
                Language::En => "d  Remove site",
            }),
            Line::from(match self.i18n.language() {
                Language::Es => "↑ ↓  Mover selección",
                Language::En => "↑ ↓  Move selection",
            }),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(match self.i18n.language() {
                    Language::Es => "Gestión",
                    Language::En => "Manage",
                }),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_schedule(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let rows = [
            format!(
                "{}: {:02}:00",
                self.i18n.start_label(),
                self.config.schedule.start
            ),
            format!(
                "{}: {:02}:00",
                self.i18n.end_label(),
                self.config.schedule.end
            ),
            format!(
                "{}: {}",
                self.i18n.weekends_label(),
                match self.i18n.language() {
                    Language::Es => {
                        if self.config.schedule.block_weekends {
                            "activados"
                        } else {
                            "desactivados"
                        }
                    }
                    Language::En => {
                        if self.config.schedule.block_weekends {
                            "on"
                        } else {
                            "off"
                        }
                    }
                }
            ),
        ];

        let items = rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let style = if index == self.schedule_cursor {
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(row.clone(), style)))
            })
            .collect::<Vec<_>>();

        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.schedule_tab()),
            ),
            columns[0],
        );

        let side = Paragraph::new(vec![
            Line::from(self.i18n.schedule_summary(
                self.config.schedule.start,
                self.config.schedule.end,
                self.config.schedule.block_weekends,
            )),
            Line::from(""),
            Line::from(self.i18n.schedule_controls()),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(match self.i18n.language() {
                    Language::Es => "Control",
                    Language::En => "Controls",
                }),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_settings(&self, frame: &mut Frame, area: Rect) {
        let monk_status = if self.config.monk_mode {
            Span::styled(
                self.i18n.monk_mode_active_label(),
                Style::default().fg(Color::Rgb(224, 102, 102)).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                self.i18n.monk_mode_inactive_label(),
                Style::default().fg(Color::Rgb(177, 214, 166)),
            )
        };

        let content = vec![
            Line::from(format!(
                "{}: {} ({})",
                self.i18n.language_label(),
                self.prefs.language.native_name(),
                self.prefs.language.code()
            )),
            Line::from(match self.i18n.language() {
                Language::Es => "← → cambian el idioma",
                Language::En => "← → changes the language",
            }),
            Line::from(""),
            Line::from(vec![
                Span::raw(format!("{}: ", self.i18n.monk_mode_label())),
                monk_status,
            ]),
            Line::from(self.i18n.monk_mode_toggle_hint()),
            Line::from(""),
            Line::from(self.i18n.install_note()),
        ];

        frame.render_widget(
            Paragraph::new(content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.settings_tab()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_diagnostics(&mut self, frame: &mut Frame, area: Rect) {
        let items = self
            .diagnostics
            .iter()
            .map(|item| {
                let (label, color) = match item.level {
                    DiagnosticLevel::Ok => (self.i18n.ok(), Color::Rgb(123, 201, 111)),
                    DiagnosticLevel::Warn => (self.i18n.warning(), Color::Rgb(241, 203, 126)),
                    DiagnosticLevel::Error => (self.i18n.error(), Color::Rgb(224, 102, 102)),
                };

                let mut lines = vec![
                    Line::from(Span::styled(
                        format!("[{}] {}", label, item.title),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(item.detail.clone()),
                ];

                if let Some(hint) = &item.hint {
                    lines.push(Line::from(hint.clone()));
                }

                ListItem::new(lines)
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.doctor_title()),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("• ");

        frame.render_stateful_widget(list, area, &mut self.diagnostics_state);
    }

    fn render_exit(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(Span::styled(
                self.i18n.exit_screen_title(),
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(self.i18n.exit_screen_body()),
            Line::from(self.i18n.exit_screen_hint()),
        ];

        frame.render_widget(
            Paragraph::new(content)
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.exit_tab()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_wall(&mut self, frame: &mut Frame, area: Rect) {
        let items = if self.wall_entries.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                self.i18n.wall_empty(),
                Style::default().fg(Color::Rgb(177, 214, 166))),
            ))]
        } else {
            self.wall_entries
                .iter()
                .map(|entry| {
                    let timestamp = entry.timestamp.format("%d/%m/%Y %H:%M").to_string();
                    ListItem::new(vec![
                        Line::from(Span::styled(
                            timestamp,
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        )),
                        Line::from(Span::styled(
                            format!("  {}", entry.reason),
                            Style::default()
                                .fg(Color::Rgb(224, 102, 102))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                    ])
                })
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.wall_title()),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("› ");

        frame.render_stateful_widget(list, area, &mut self.wall_state);
    }

    fn render_unblock_flow(&self, frame: &mut Frame) {
        match &self.unblock_flow {
            None => {}
            Some(UnblockFlow::Countdown { started, duration_secs }) => {
                let elapsed = started.elapsed().as_secs().min(*duration_secs);
                let remaining = duration_secs - elapsed;
                let phrase_index = (elapsed / 3) as usize % DISTRACTION_PHRASES.len();
                let phrase = DISTRACTION_PHRASES[phrase_index];

                let area = centered_rect(62, 55, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.countdown_title(),
                            Style::default()
                                .fg(Color::Rgb(241, 203, 126))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.countdown_subtitle(),
                            Style::default().fg(Color::Rgb(231, 223, 201)),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            format!("{}", remaining),
                            Style::default()
                                .fg(Color::Rgb(224, 102, 102))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            phrase,
                            Style::default()
                                .fg(Color::Rgb(177, 214, 166))
                                .add_modifier(Modifier::BOLD | Modifier::ITALIC),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.countdown_hint(),
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        )),
                    ])
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(paths::APP_DISPLAY_NAME),
                    ),
                    area,
                );
            }
            Some(UnblockFlow::ReasonPrompt { value }) => {
                let area = centered_rect(70, 35, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(Span::styled(
                            self.i18n.reason_prompt_title(),
                            Style::default()
                                .fg(Color::Rgb(241, 203, 126))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            value.clone(),
                            Style::default()
                                .fg(Color::Rgb(231, 223, 201))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.reason_prompt_hint(),
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        )),
                    ])
                    .block(Block::default().borders(Borders::ALL).title(
                        match self.i18n.language() {
                            Language::Es => "Motivo (obligatorio)",
                            Language::En => "Reason (required)",
                        },
                    ))
                    .alignment(Alignment::Left),
                    area,
                );
            }
            Some(UnblockFlow::FinalCountdown { started, reason }) => {
                let elapsed = started.elapsed().as_secs().min(60);
                let remaining = 60 - elapsed;

                // Ola animada: frame basado en tiempo real en ms
                let frame_idx = (started.elapsed().as_millis() / 120) as usize
                    % WAVE_FRAMES.len();
                let wave = WAVE_FRAMES[frame_idx];

                // Barra de progreso manual con bloques
                let bar_width: usize = 30;
                let filled = ((elapsed as usize) * bar_width) / 60;
                let empty = bar_width - filled;
                let bar = format!(
                    "[{}{}] {}s",
                    "█".repeat(filled),
                    "░".repeat(empty),
                    remaining,
                );

                // Inhala / exhala: ciclo de 4s
                let breath = if (elapsed / 4) % 2 == 0 {
                    self.i18n.breath_in()
                } else {
                    self.i18n.breath_out()
                };

                let area = centered_rect(68, 60, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.final_countdown_title(),
                            Style::default()
                                .fg(Color::Rgb(241, 203, 126))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(vec![
                            Span::styled(
                                format!("{}:  ", self.i18n.final_countdown_reason_label()),
                                Style::default().fg(Color::Rgb(190, 197, 208)),
                            ),
                            Span::styled(
                                format!("\"{}\"", reason),
                                Style::default()
                                    .fg(Color::Rgb(224, 102, 102))
                                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
                            ),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled(
                            breath,
                            Style::default()
                                .fg(Color::Rgb(177, 214, 166))
                                .add_modifier(Modifier::BOLD),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            wave,
                            Style::default().fg(Color::Rgb(177, 214, 166)),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            bar,
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        )),
                        Line::from(""),
                        Line::from(Span::styled(
                            self.i18n.countdown_hint(),
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        )),
                    ])
                    .alignment(Alignment::Center)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(paths::APP_DISPLAY_NAME),
                    ),
                    area,
                );
            }
        }
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let fallback = match self.active_tab {
            TabId::Diagnostics => self.i18n.diagnostics_refresh().to_string(),
            TabId::Exit => self.i18n.exit_screen_body().to_string(),
            _ => self.i18n.tui_hint().to_string(),
        };
        let message = self.flash_message.clone().unwrap_or(fallback);

        let paragraph = Paragraph::new(message)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_prompt(&self, frame: &mut Frame, prompt: &PromptState) {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(prompt.title.clone()),
                Line::from(""),
                Line::from(Span::styled(
                    prompt.value.clone(),
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match self.i18n.language() {
                        Language::Es => "Entrada",
                        Language::En => "Input",
                    }),
            )
            .alignment(Alignment::Left),
            area,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.unblock_flow.is_some() {
            return self.handle_unblock_flow_key(key);
        }

        if self.remove_flow.is_some() {
            return self.handle_remove_flow_key(key);
        }

        if self.prompt.is_some() {
            return self.handle_prompt_key(key);
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),
            KeyCode::Char('b') => self.try_block()?,
            KeyCode::Char('u') => self.try_unblock()?,
            KeyCode::Char('r') => self.refresh_status()?,
            _ => return self.handle_tab_key(key),
        }

        Ok(false)
    }

    fn handle_tab_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.active_tab {
            TabId::Home => {}
            TabId::Sites => self.handle_sites_key(key)?,
            TabId::Schedule => self.handle_schedule_key(key)?,
            TabId::Settings => self.handle_settings_key(key)?,
            TabId::Diagnostics => self.handle_diagnostics_key(key)?,
            TabId::Wall => self.handle_wall_key(key),
            TabId::Exit => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn handle_sites_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down => self.select_next_site(),
            KeyCode::Up => self.select_previous_site(),
            KeyCode::Char('a') => {
                self.prompt = Some(PromptState {
                    title: self.i18n.add_site_prompt().to_string(),
                    value: String::new(),
                });
            }
            KeyCode::Char('d') => self.start_remove_confirm(),
            _ => {}
        }

        Ok(())
    }

    fn handle_schedule_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Down => self.schedule_cursor = (self.schedule_cursor + 1) % 3,
            KeyCode::Up => self.schedule_cursor = (self.schedule_cursor + 2) % 3,
            KeyCode::Left => self.adjust_schedule(-1)?,
            KeyCode::Right => self.adjust_schedule(1)?,
            _ => {}
        }

        Ok(())
    }

    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Enter => self.toggle_language()?,
            KeyCode::Char('m') => self.toggle_monk_mode()?,
            _ => {}
        }

        Ok(())
    }

    fn handle_diagnostics_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('g') => self.refresh_diagnostics()?,
            KeyCode::Down => self.select_next_diagnostic(),
            KeyCode::Up => self.select_previous_diagnostic(),
            _ => {}
        }

        Ok(())
    }

    fn handle_wall_key(&mut self, key: KeyEvent) {
        if self.wall_entries.is_empty() {
            return;
        }
        let len = self.wall_entries.len();
        match key.code {
            KeyCode::Down => {
                let next = match self.wall_state.selected() {
                    Some(i) => (i + 1) % len,
                    None => 0,
                };
                self.wall_state.select(Some(next));
            }
            KeyCode::Up => {
                let next = match self.wall_state.selected() {
                    Some(i) => (i + len - 1) % len,
                    None => 0,
                };
                self.wall_state.select(Some(next));
            }
            _ => {}
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) -> Result<bool> {
        let prompt = self.prompt.as_mut().expect("prompt must exist");
        match key.code {
            KeyCode::Esc => {
                self.prompt = None;
            }
            KeyCode::Enter => {
                let value = prompt.value.clone();
                self.prompt = None;
                self.add_site(value.trim())?;
            }
            KeyCode::Backspace => {
                prompt.value.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                prompt.value.push(ch);
            }
            _ => {}
        }

        Ok(false)
    }

    fn try_block(&mut self) -> Result<()> {
        match ipc::send_command("block") {
            Ok(()) => self.set_flash(self.i18n.blocked_now()),
            Err(error) => self.set_flash(error),
        }
        self.refresh_status()?;
        Ok(())
    }

    fn try_unblock(&mut self) -> Result<()> {
        if self.status.monk_mode {
            self.set_flash(self.i18n.monk_mode_no_unblock());
            return Ok(());
        }

        if self.status.schedule_active {
            let prior = journal::count_today();
            let duration_secs: u64 = match prior {
                0 => 30,
                1 => 180,
                2 => 600,
                _ => 1800,
            };
            if prior > 0 {
                self.set_flash(self.i18n.unlock_attempt_warning(prior));
            }
            self.unblock_flow = Some(UnblockFlow::Countdown {
                started: Instant::now(),
                duration_secs,
            });
        } else {
            match ipc::send_command("unblock") {
                Ok(()) => self.set_flash(self.i18n.unblocked_now()),
                Err(error) => self.set_flash(error),
            }
            self.refresh_status()?;
        }
        Ok(())
    }

    fn advance_unblock_flow(&mut self) {
        // Countdown → ReasonPrompt (duration depends on attempt count)
        let to_reason = match &self.unblock_flow {
            Some(UnblockFlow::Countdown { started, duration_secs }) => {
                started.elapsed().as_secs() >= *duration_secs
            }
            _ => false,
        };
        if to_reason {
            self.unblock_flow = Some(UnblockFlow::ReasonPrompt {
                value: String::new(),
            });
            return;
        }

        // FinalCountdown (60s) → pending_unblock (side-effects en perform_pending_actions)
        let final_done = match &self.unblock_flow {
            Some(UnblockFlow::FinalCountdown { started, .. }) => {
                started.elapsed().as_secs() >= 60
            }
            _ => false,
        };
        if final_done {
            let reason = match self.unblock_flow.take() {
                Some(UnblockFlow::FinalCountdown { reason, .. }) => reason,
                _ => String::new(),
            };
            self.pending_unblock = Some(reason);
        }
    }

    fn handle_unblock_flow_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Esc siempre cancela el flow
        if matches!(key.code, KeyCode::Esc) {
            self.unblock_flow = None;
            self.set_flash(self.i18n.unblock_cancelled());
            return Ok(false);
        }

        let is_reason_prompt = matches!(
            self.unblock_flow,
            Some(UnblockFlow::ReasonPrompt { .. })
        );

        if is_reason_prompt {
            match key.code {
                KeyCode::Enter => {
                    let reason = match &self.unblock_flow {
                        Some(UnblockFlow::ReasonPrompt { value }) => value.trim().to_string(),
                        _ => String::new(),
                    };
                    if reason.is_empty() {
                        self.set_flash(self.i18n.reason_required());
                    } else {
                        self.unblock_flow = Some(UnblockFlow::FinalCountdown {
                            started: Instant::now(),
                            reason,
                        });
                    }
                }
                KeyCode::Backspace => {
                    if let Some(UnblockFlow::ReasonPrompt { value }) = &mut self.unblock_flow {
                        value.pop();
                    }
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(UnblockFlow::ReasonPrompt { value }) = &mut self.unblock_flow {
                        value.push(ch);
                    }
                }
                _ => {}
            }
        }

        Ok(false)
    }

    fn add_site(&mut self, raw: &str) -> Result<()> {
        match self.config.add_site(raw) {
            Ok(SiteMutation::Added(site)) => {
                self.config.save_active()?;
                match ipc::send_command("sync") {
                    Ok(()) => self.set_flash(self.i18n.site_added(&site)),
                    Err(error) => self.set_flash(error),
                }
                self.refresh_status()?;
                self.refresh_diagnostics()?;
                self.sites_state.select(
                    self.config
                        .sites
                        .list
                        .iter()
                        .position(|entry| entry == &site),
                );
            }
            Ok(SiteMutation::AlreadyPresent(site)) => {
                self.set_flash(self.i18n.site_already_present(&site))
            }
            Ok(_) => {}
            Err(error) => self.set_flash(error),
        }

        Ok(())
    }

    fn remove_selected_site(&mut self) -> Result<()> {
        let selected = self
            .sites_state
            .selected()
            .and_then(|index| self.config.sites.list.get(index))
            .cloned();

        let Some(site) = selected else {
            self.set_flash(self.i18n.site_empty());
            return Ok(());
        };

        match self.config.remove_site(&site) {
            Ok(SiteMutation::Removed(site)) => {
                self.config.save_active()?;
                match ipc::send_command("sync") {
                    Ok(()) => self.set_flash(self.i18n.site_removed(&site)),
                    Err(error) => self.set_flash(error),
                }
                self.refresh_status()?;
                self.refresh_diagnostics()?;
                if self.config.sites.list.is_empty() {
                    self.sites_state.select(None);
                } else if let Some(index) = self.sites_state.selected() {
                    let next = index.min(self.config.sites.list.len() - 1);
                    self.sites_state.select(Some(next));
                }
            }
            Ok(SiteMutation::NotFound(site)) => self.set_flash(self.i18n.site_not_found(&site)),
            Ok(_) => {}
            Err(error) => self.set_flash(error),
        }

        Ok(())
    }

    fn adjust_schedule(&mut self, delta: i8) -> Result<()> {
        let mut start = self.config.schedule.start;
        let mut end = self.config.schedule.end;
        let mut weekends = self.config.schedule.block_weekends;

        match self.schedule_cursor {
            0 => start = wrap_hour(start, delta),
            1 => end = wrap_hour(end, delta),
            2 => weekends = !weekends,
            _ => {}
        }

        if let Err(error) = self.config.set_schedule(start, end, weekends) {
            self.set_flash(error);
            return Ok(());
        }

        self.config.save_active()?;
        match ipc::send_command("sync") {
            Ok(()) => self.set_flash(self.i18n.schedule_updated(start, end, weekends)),
            Err(error) => self.set_flash(error),
        }
        self.refresh_status()?;
        self.refresh_diagnostics()?;

        Ok(())
    }

    fn toggle_language(&mut self) -> Result<()> {
        self.prefs.language = match self.prefs.language {
            Language::Es => Language::En,
            Language::En => Language::Es,
        };
        self.prefs.save()?;
        self.i18n = I18n::new(self.prefs.language);
        self.set_flash(self.i18n.language_updated(self.prefs.language));
        self.refresh_diagnostics()?;
        Ok(())
    }

    fn toggle_monk_mode(&mut self) -> Result<()> {
        self.config.monk_mode = !self.config.monk_mode;
        self.config.save_active()?;
        match ipc::send_command("sync") {
            Ok(()) => {
                if self.config.monk_mode {
                    self.set_flash(self.i18n.monk_mode_activated());
                } else {
                    self.set_flash(self.i18n.monk_mode_deactivated());
                }
            }
            Err(error) => self.set_flash(error),
        }
        self.refresh_status()?;
        Ok(())
    }

    fn start_remove_confirm(&mut self) {
        let selected = self
            .sites_state
            .selected()
            .and_then(|i| self.config.sites.list.get(i))
            .cloned();
        if let Some(site) = selected {
            self.remove_flow = Some(RemoveConfirmFlow::Typing {
                site,
                typed: String::new(),
            });
        } else {
            self.set_flash(self.i18n.site_empty());
        }
    }

    fn handle_remove_flow_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.code, KeyCode::Esc) {
            self.remove_flow = None;
            self.set_flash(self.i18n.unblock_cancelled());
            return Ok(false);
        }

        if let Some(RemoveConfirmFlow::Typing { site, typed }) = &mut self.remove_flow {
            match key.code {
                KeyCode::Enter => {
                    if *typed == *site {
                        let site_clone = site.clone();
                        self.remove_flow = None;
                        self.remove_selected_site_confirmed(&site_clone)?;
                    } else {
                        self.remove_flow = None;
                        self.set_flash(self.i18n.remove_confirm_wrong());
                    }
                }
                KeyCode::Backspace => {
                    typed.pop();
                }
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    typed.push(ch);
                }
                _ => {}
            }
        }

        Ok(false)
    }

    fn remove_selected_site_confirmed(&mut self, site: &str) -> Result<()> {
        match self.config.remove_site(site) {
            Ok(SiteMutation::Removed(removed)) => {
                self.config.save_active()?;
                match ipc::send_command("sync") {
                    Ok(()) => self.set_flash(self.i18n.site_removed(&removed)),
                    Err(error) => self.set_flash(error),
                }
                self.refresh_status()?;
                self.refresh_diagnostics()?;
                if self.config.sites.list.is_empty() {
                    self.sites_state.select(None);
                } else if let Some(index) = self.sites_state.selected() {
                    let next = index.min(self.config.sites.list.len() - 1);
                    self.sites_state.select(Some(next));
                }
            }
            Ok(SiteMutation::NotFound(s)) => self.set_flash(self.i18n.site_not_found(&s)),
            Ok(_) => {}
            Err(error) => self.set_flash(error),
        }
        Ok(())
    }

    fn render_remove_flow(&self, frame: &mut Frame) {
        let Some(RemoveConfirmFlow::Typing { site, typed }) = &self.remove_flow else {
            return;
        };

        let area = centered_rect(68, 35, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    self.i18n.remove_confirm_prompt(site),
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    typed.clone(),
                    Style::default()
                        .fg(Color::Rgb(231, 223, 201))
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    match self.i18n.language() {
                        Language::Es => "Enter para confirmar · Esc para cancelar",
                        Language::En => "Enter to confirm · Esc to cancel",
                    },
                    Style::default().fg(Color::Rgb(190, 197, 208)),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match self.i18n.language() {
                        Language::Es => "Confirmar eliminación",
                        Language::En => "Confirm removal",
                    }),
            )
            .alignment(Alignment::Left),
            area,
        );
    }

    pub fn perform_pending_actions(&mut self) -> Result<()> {
        if let Some(reason) = self.pending_unblock.take() {
            let _ = journal::append(&reason);
            self.wall_entries = journal::load().unwrap_or_default();
            match ipc::send_command("unblock") {
                Ok(()) => self.set_flash(self.i18n.unblocked_now()),
                Err(error) => self.set_flash(error),
            }
            self.refresh_status()?;
        }
        Ok(())
    }

    fn refresh_status(&mut self) -> Result<()> {
        self.status = blocker::status_snapshot(&self.config)?;
        Ok(())
    }

    fn refresh_diagnostics(&mut self) -> Result<()> {
        self.diagnostics = doctor::run(&self.config, self.i18n)?;
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
        } else if self.diagnostics_state.selected().is_none() {
            self.diagnostics_state.select(Some(0));
        }
        Ok(())
    }

    fn set_flash(&mut self, message: impl Into<String>) {
        self.flash_message = Some(message.into());
    }

    fn next_tab(&mut self) {
        self.active_tab = TabId::from_index(self.active_tab.index() + 1);
    }

    fn previous_tab(&mut self) {
        let count = TabId::all().len();
        self.active_tab = TabId::from_index((self.active_tab.index() + count - 1) % count);
    }

    fn select_next_site(&mut self) {
        if self.config.sites.list.is_empty() {
            self.sites_state.select(None);
            return;
        }

        let next = match self.sites_state.selected() {
            Some(index) => (index + 1) % self.config.sites.list.len(),
            None => 0,
        };
        self.sites_state.select(Some(next));
    }

    fn select_previous_site(&mut self) {
        if self.config.sites.list.is_empty() {
            self.sites_state.select(None);
            return;
        }

        let len = self.config.sites.list.len();
        let next = match self.sites_state.selected() {
            Some(index) => (index + len - 1) % len,
            None => 0,
        };
        self.sites_state.select(Some(next));
    }

    fn select_next_diagnostic(&mut self) {
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
            return;
        }

        let next = match self.diagnostics_state.selected() {
            Some(index) => (index + 1) % self.diagnostics.len(),
            None => 0,
        };
        self.diagnostics_state.select(Some(next));
    }

    fn select_previous_diagnostic(&mut self) {
        if self.diagnostics.is_empty() {
            self.diagnostics_state.select(None);
            return;
        }

        let len = self.diagnostics.len();
        let next = match self.diagnostics_state.selected() {
            Some(index) => (index + len - 1) % len,
            None => 0,
        };
        self.diagnostics_state.select(Some(next));
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

fn wrap_hour(value: u8, delta: i8) -> u8 {
    let raw = value as i16 + delta as i16;
    if raw < 0 {
        23
    } else if raw > 23 {
        0
    } else {
        raw as u8
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
