use super::animations::{ASCII_ART, DISTRACTION_PHRASES, WAVE_FRAMES};
use super::helpers::centered_rect;
use super::state::{Dashboard, FrictionAction, FrictionFlow, HabitsInput, TabId};
use crate::attempts::DayAttempts;
use crate::doctor::DiagnosticLevel;
use crate::habits::{self, HabitFrequency, Habit};
use crate::i18n::I18n;
use crate::paths;
use chrono::Datelike;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

impl Dashboard {
    pub(crate) fn render(&mut self, frame: &mut Frame) {
        self.advance_friction_flow();

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

        if let Some(prompt) = &self.prompt.clone() {
            self.render_prompt(frame, &prompt.title.clone(), &prompt.value.clone());
        }

        // Habits modals (drawn after body so they float on top)
        match self.habits_input.clone() {
            Some(HabitsInput::EnteringName { value }) => {
                self.render_prompt(frame, self.i18n.habits_add_name_prompt(), &value);
            }
            Some(HabitsInput::SelectingFrequency { selected, .. }) => {
                self.render_habits_freq_modal(frame, selected);
            }
            Some(HabitsInput::ConfirmDelete { .. }) => {
                self.render_prompt(frame, self.i18n.habits_delete_confirm(), "");
            }
            None => {}
        }

        self.render_friction_flow(frame);
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

        let subtitle = self.i18n.subtitle();

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
                format!(
                    "{}: {} {}",
                    self.i18n.streak_header(),
                    self.streak.current,
                    self.i18n.streak_days()
                ),
                Style::default()
                    .fg(Color::Rgb(177, 214, 166))
                    .add_modifier(Modifier::BOLD),
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
            self.i18n.attempts_tab(),
            self.i18n.streak_tab(),
            self.i18n.habits_tab(),
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
            TabId::Attempts => self.render_attempts(frame, area),
            TabId::Streak => self.render_streak(frame, area),
            TabId::Habits => self.render_habits(frame, area),
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

        use chrono::Datelike;
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
        ];

        let status_panel = Paragraph::new(home_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.status_title()),
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
            Line::from(format!("{}: {}", self.i18n.selected_label(), selected_site)),
            Line::from(""),
            Line::from(self.i18n.site_add_action()),
            Line::from(self.i18n.site_remove_action()),
            Line::from(self.i18n.site_move_selection()),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(self.i18n.manage_label()),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_schedule(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        let weekends_value = if self.config.schedule.block_weekends {
            self.i18n.weekends_on()
        } else {
            self.i18n.weekends_off()
        };

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
            format!("{}: {}", self.i18n.weekends_label(), weekends_value),
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
                .title(self.i18n.controls_label()),
        )
        .wrap(Wrap { trim: true });

        frame.render_widget(side, columns[1]);
    }

    fn render_settings(&self, frame: &mut Frame, area: Rect) {
        let content = vec![
            Line::from(format!(
                "{}: {} ({})",
                self.i18n.language_label(),
                self.prefs.language.native_name(),
                self.prefs.language.code()
            )),
            Line::from(self.i18n.language_change_hint()),
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

    fn render_streak(&self, frame: &mut Frame, area: Rect) {
        let last_break = self
            .streak
            .last_break
            .map(|date| date.format("%d/%m/%Y").to_string())
            .unwrap_or_else(|| self.i18n.streak_never_broken().to_string());
        let last_break_reason = self
            .streak
            .last_break_reason
            .clone()
            .unwrap_or_else(|| self.i18n.streak_never_broken().to_string());

        let content = vec![
            Line::from(format!(
                "{}: {} {}",
                self.i18n.streak_current(),
                self.streak.current,
                self.i18n.streak_days()
            )),
            Line::from(format!(
                "{}: {} {}",
                self.i18n.streak_best(),
                self.streak.best,
                self.i18n.streak_days()
            )),
            Line::from(format!("{}: {}", self.i18n.streak_last_break(), last_break)),
            Line::from(format!(
                "{}: {}",
                self.i18n.streak_last_break_reason(),
                last_break_reason
            )),
            Line::from(format!(
                "{}: {}",
                self.i18n.streak_total_breaks(),
                self.streak.total_breaks
            )),
        ];

        frame.render_widget(
            Paragraph::new(content)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.streak_tab()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_attempts(&self, frame: &mut Frame, area: Rect) {
        let week = self.attempts_last_7_days.iter().fold(
            DayAttempts {
                date: self.attempts_today.date,
                initiated: 0,
                completed: 0,
                resisted: 0,
            },
            |mut totals, day| {
                totals.initiated += day.initiated;
                totals.completed += day.completed;
                totals.resisted += day.resisted;
                totals
            },
        );

        let mut lines = vec![
            Line::from(Span::styled(
                self.i18n.attempts_title(),
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("─────────────────────────────────────"),
            self.attempts_summary_line(self.i18n.attempts_today(), &self.attempts_today),
            self.attempts_summary_line(self.i18n.attempts_week(), &week),
            Line::from("─────────────────────────────────────"),
            Line::from(format!("{}:", self.i18n.attempts_last_days())),
        ];

        for day in &self.attempts_last_7_days {
            lines.push(self.attempts_day_line(day));
        }

        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_wall(&mut self, frame: &mut Frame, area: Rect) {
        let items = if self.wall_entries.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                self.i18n.wall_empty(),
                Style::default().fg(Color::Rgb(177, 214, 166)),
            )))]
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

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let fallback = match self.active_tab {
            TabId::Diagnostics => self.i18n.diagnostics_refresh().to_string(),
            TabId::Exit => self.i18n.exit_screen_body().to_string(),
            TabId::Habits => "[Enter] marcar/desmarcar  [a] añadir  [d] eliminar  ↑↓ navegar".to_string(),
            _ => self.i18n.tui_hint().to_string(),
        };
        let message = self.flash_message.clone().unwrap_or(fallback);

        let paragraph = Paragraph::new(message)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    fn render_prompt(&self, frame: &mut Frame, title: &str, value: &str) {
        let area = centered_rect(60, 20, frame.area());
        frame.render_widget(Clear, area);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(title.to_string()),
                Line::from(""),
                Line::from(Span::styled(
                    value.to_string(),
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD),
                )),
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.input_label()),
            )
            .alignment(Alignment::Left),
            area,
        );
    }

    fn friction_countdown_title(&self, action: &FrictionAction) -> String {
        match action {
            FrictionAction::Unblock => self.i18n.countdown_title().to_string(),
            FrictionAction::RemoveSite(site) => {
                self.i18n.format("site_remove_countdown_title", &[site])
            }
        }
    }

    fn friction_countdown_subtitle(&self, action: &FrictionAction) -> String {
        match action {
            FrictionAction::Unblock => self.i18n.countdown_subtitle().to_string(),
            FrictionAction::RemoveSite(_) => {
                self.i18n.t("site_remove_countdown_subtitle").to_string()
            }
        }
    }

    fn friction_reason_title(&self, action: &FrictionAction) -> String {
        match action {
            FrictionAction::Unblock => self.i18n.reason_prompt_title().to_string(),
            FrictionAction::RemoveSite(site) => {
                self.i18n.format("site_remove_reason_title", &[site])
            }
        }
    }

    fn friction_final_title(&self, action: &FrictionAction) -> String {
        match action {
            FrictionAction::Unblock => self.i18n.final_countdown_title().to_string(),
            FrictionAction::RemoveSite(site) => {
                self.i18n.format("site_remove_final_title", &[site])
            }
        }
    }

    fn render_friction_flow(&self, frame: &mut Frame) {
        match &self.friction_flow {
            None => {}
            Some(FrictionFlow::Countdown { action, started }) => {
                let elapsed = started.elapsed().as_secs().min(30);
                let remaining = 30 - elapsed;
                let phrase_index = (elapsed / 3) as usize % DISTRACTION_PHRASES.len();
                let phrase = DISTRACTION_PHRASES[phrase_index];

                let title = self.friction_countdown_title(action);
                let subtitle = self.friction_countdown_subtitle(action);

                let area = centered_rect(62, 55, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(""),
                        Line::from(Span::styled(
                            title,
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
            Some(FrictionFlow::ReasonPrompt { action, value }) => {
                let title = self.friction_reason_title(action);
                let area = centered_rect(70, 35, frame.area());
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(Span::styled(
                            title,
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
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.i18n.reason_required_label()),
                    )
                    .alignment(Alignment::Left),
                    area,
                );
            }
            Some(FrictionFlow::FinalCountdown {
                action,
                started,
                reason,
            }) => {
                let elapsed = started.elapsed().as_secs().min(60);
                let remaining = 60 - elapsed;

                let frame_idx = (started.elapsed().as_millis() / 120) as usize % WAVE_FRAMES.len();
                let wave = WAVE_FRAMES[frame_idx];

                let bar_width: usize = 30;
                let filled = ((elapsed as usize) * bar_width) / 60;
                let empty = bar_width - filled;
                let bar = format!(
                    "[{}{}] {}s",
                    "█".repeat(filled),
                    "░".repeat(empty),
                    remaining,
                );

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
                            self.friction_final_title(action),
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

    fn render_habits(&mut self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(area);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(rows[0]);

        self.render_habits_list(frame, columns[0]);
        self.render_habits_detail(frame, columns[1]);
        self.render_habits_progress_strip(frame, rows[1]);
    }

    fn render_habits_list(&mut self, frame: &mut Frame, area: Rect) {
        let i18n = self.i18n;
        let items: Vec<ListItem> = if self.habits.habits.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                i18n.habits_empty(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            )))]
        } else {
            self.habits
                .habits
                .iter()
                .map(|h| {
                    let (icon, icon_color) = habit_status_icon(h);
                    let freq = habit_freq_label(h, i18n);
                    let name = truncate_habit_name(&h.name, 22);
                    let line = Line::from(vec![
                        Span::styled(
                            format!("{} ", icon),
                            Style::default().fg(icon_color),
                        ),
                        Span::raw(format!("{:<22} ", name)),
                        Span::styled(
                            freq.to_string(),
                            Style::default().fg(Color::Rgb(190, 197, 208)),
                        ),
                    ]);
                    ListItem::new(line)
                })
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.i18n.habits_title()),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(241, 203, 126))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("› ");

        frame.render_stateful_widget(list, area, &mut self.habits_state);
    }

    fn render_habits_detail(&self, frame: &mut Frame, area: Rect) {
        let selected = self
            .habits_state
            .selected()
            .and_then(|i| self.habits.habits.get(i));

        let lines: Vec<Line> = match selected {
            None => vec![Line::from(Span::styled(
                self.i18n.habits_none_selected(),
                Style::default().fg(Color::Rgb(190, 197, 208)),
            ))],
            Some(h) => {
                let today = chrono::Local::now().date_naive();
                let streak = habits::current_streak(h);
                let best = habits::best_streak(h);
                let rate = habits::completion_rate_last_n(h, 14);
                let done_today = habits::is_completed_in_period(h, today);

                let bar: String = (0u32..14)
                    .rev()
                    .map(|offset| {
                        let date = today - chrono::Duration::days(offset as i64);
                        if matches!(h.frequency, HabitFrequency::Weekdays)
                            && matches!(
                                date.weekday(),
                                chrono::Weekday::Sat | chrono::Weekday::Sun
                            )
                        {
                            '·'
                        } else if habits::is_completed_in_period(h, date) {
                            '█'
                        } else {
                            '░'
                        }
                    })
                    .collect();

                let freq_label = habit_freq_label(h, self.i18n);

                vec![
                    Line::from(Span::styled(
                        h.name.clone(),
                        Style::default()
                            .fg(Color::Rgb(241, 203, 126))
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(format!(
                        "{}: {}",
                        self.i18n.habits_frequency(),
                        freq_label
                    )),
                    Line::from(format!(
                        "{}: {}",
                        self.i18n.habits_streak_current(),
                        streak
                    )),
                    Line::from(format!(
                        "{}: {}",
                        self.i18n.habits_streak_best(),
                        best
                    )),
                    Line::from(format!(
                        "{}: {}",
                        self.i18n.habits_done_today(),
                        if done_today {
                            self.i18n.habits_si()
                        } else {
                            self.i18n.habits_no()
                        }
                    )),
                    Line::from(format!(
                        "{}: {}",
                        self.i18n.habits_created(),
                        h.created_at
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        self.i18n.habits_last_14(),
                        Style::default().fg(Color::Rgb(190, 197, 208)),
                    )),
                    Line::from(Span::styled(
                        bar,
                        Style::default().fg(Color::Rgb(123, 201, 111)),
                    )),
                    Line::from(Span::styled(
                        format!("{}%", rate),
                        Style::default().fg(Color::Rgb(177, 214, 166)),
                    )),
                ]
            }
        };

        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.habits_detail_title()),
                )
                .wrap(Wrap { trim: true }),
            area,
        );
    }

    fn render_habits_progress_strip(&self, frame: &mut Frame, area: Rect) {
        let today = chrono::Local::now().date_naive();
        let due_count = self
            .habits
            .habits
            .iter()
            .filter(|h| habits::is_due_today(h))
            .count();
        let done_count = self
            .habits
            .habits
            .iter()
            .filter(|h| habits::is_due_today(h) && habits::is_completed_in_period(h, today))
            .count();

        let bar_width: usize = 20;
        let filled = if due_count > 0 {
            (done_count * bar_width) / due_count
        } else {
            0
        };
        let empty = bar_width - filled;
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        let color = if due_count == 0 {
            Color::Rgb(190, 197, 208)
        } else if done_count == due_count {
            Color::Rgb(123, 201, 111)
        } else {
            Color::Rgb(241, 203, 126)
        };

        let line = Line::from(vec![
            Span::styled(
                format!(
                    "{}: {}/{} ",
                    self.i18n.habits_today_progress(),
                    done_count,
                    due_count
                ),
                Style::default().fg(Color::Rgb(231, 223, 201)),
            ),
            Span::styled(bar, Style::default().fg(color)),
        ]);

        frame.render_widget(
            Paragraph::new(line)
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_habits_freq_modal(&self, frame: &mut Frame, selected: usize) {
        let area = centered_rect(50, 45, frame.area());
        frame.render_widget(Clear, area);

        let options = [
            self.i18n.habits_freq_option_daily(),
            self.i18n.habits_freq_option_weekly(),
            self.i18n.habits_freq_option_monthly(),
            self.i18n.habits_freq_option_weekdays(),
        ];

        let mut lines = vec![
            Line::from(Span::styled(
                self.i18n.habits_add_freq_prompt(),
                Style::default().fg(Color::Rgb(231, 223, 201)),
            )),
            Line::from(""),
        ];

        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                lines.push(Line::from(Span::styled(
                    format!("› {}", opt),
                    Style::default()
                        .fg(Color::Rgb(241, 203, 126))
                        .add_modifier(Modifier::BOLD),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {}", opt),
                    Style::default().fg(Color::Rgb(190, 197, 208)),
                )));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Enter confirmar  ·  Esc cancelar",
            Style::default().fg(Color::Rgb(190, 197, 208)),
        )));

        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(self.i18n.habits_add_freq_prompt()),
                )
                .alignment(Alignment::Left),
            area,
        );
    }

    fn attempts_summary_line(&self, label: &str, day: &DayAttempts) -> Line<'static> {
        Line::from(format!(
            "{label:<12} {}: {}   {}: {}   {}: {}",
            self.i18n.attempts_initiated(),
            day.initiated,
            self.i18n.attempts_resisted(),
            day.resisted,
            self.i18n.attempts_completed(),
            day.completed
        ))
    }

    fn attempts_day_line(&self, day: &DayAttempts) -> Line<'static> {
        let label = self.short_weekday(day.date.weekday());
        let filled = ((day.resisted * 6) + (day.initiated / 2))
            .checked_div(day.initiated)
            .unwrap_or(0);
        let empty = 6_u32.saturating_sub(filled);
        let bar = format!(
            "{}{}",
            "█".repeat(filled as usize),
            "░".repeat(empty as usize)
        );

        let detail = if day.initiated == 0 {
            format!("0  ← {}", self.i18n.attempts_no_attempts())
        } else if day.resisted == day.initiated {
            format!(
                "{}/{}  ← {}",
                day.resisted,
                day.initiated,
                self.i18n.attempts_clean_day()
            )
        } else {
            format!("{}/{}", day.resisted, day.initiated)
        };

        let bar_color = if day.initiated == 0 {
            Color::DarkGray
        } else if day.resisted == day.initiated {
            Color::Rgb(123, 201, 111)
        } else {
            Color::Rgb(241, 203, 126)
        };

        Line::from(vec![
            Span::raw(format!("{label:<3}  ")),
            Span::styled(bar, Style::default().fg(bar_color)),
            Span::raw(format!("  {detail}")),
        ])
    }

    fn short_weekday(&self, weekday: chrono::Weekday) -> String {
        self.i18n.weekday(weekday).chars().take(3).collect()
    }
}

// Free functions to avoid borrow conflicts: these take plain refs rather than &self,
// so the list's &mut ListState can be borrowed simultaneously.

fn habit_status_icon(h: &Habit) -> (&'static str, Color) {
    let today = chrono::Local::now().date_naive();
    if !habits::is_due_today(h) {
        ("—", Color::Rgb(190, 197, 208))
    } else if habits::is_completed_in_period(h, today) {
        ("✓", Color::Rgb(123, 201, 111))
    } else {
        ("○", Color::Rgb(241, 203, 126))
    }
}

fn habit_freq_label(h: &Habit, i18n: I18n) -> &'static str {
    match h.frequency {
        HabitFrequency::Daily => i18n.habits_freq_daily(),
        HabitFrequency::Weekly => i18n.habits_freq_weekly(),
        HabitFrequency::Monthly => i18n.habits_freq_monthly(),
        HabitFrequency::Weekdays => i18n.habits_freq_weekdays(),
    }
}

fn truncate_habit_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        format!("{}…", name.chars().take(max - 1).collect::<String>())
    }
}
