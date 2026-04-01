use super::animations::{ASCII_ART, DISTRACTION_PHRASES, WAVE_FRAMES};
use super::helpers::centered_rect;
use super::state::{Dashboard, TabId, UnblockFlow};
use crate::doctor::DiagnosticLevel;
use crate::paths;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};
use ratatui::Frame;

impl Dashboard {
    pub(crate) fn render(&mut self, frame: &mut Frame) {
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

        if let Some(prompt) = &self.prompt.clone() {
            self.render_prompt(frame, &prompt.title.clone(), &prompt.value.clone());
        }

        self.render_unblock_flow(frame);
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
            Line::from(format!(
                "{}: {}",
                self.i18n.selected_label(),
                selected_site
            )),
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

    fn render_unblock_flow(&self, frame: &mut Frame) {
        match &self.unblock_flow {
            None => {}
            Some(UnblockFlow::Countdown { started }) => {
                let elapsed = started.elapsed().as_secs().min(30);
                let remaining = 30 - elapsed;
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
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(self.i18n.reason_required_label()),
                    )
                    .alignment(Alignment::Left),
                    area,
                );
            }
            Some(UnblockFlow::FinalCountdown { started, reason }) => {
                let elapsed = started.elapsed().as_secs().min(60);
                let remaining = 60 - elapsed;

                let frame_idx =
                    (started.elapsed().as_millis() / 120) as usize % WAVE_FRAMES.len();
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
}
