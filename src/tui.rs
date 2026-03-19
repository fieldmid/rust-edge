use std::{
    io,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_lite::StreamExt;
use powersync::PowerSyncDatabase;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph, Wrap},
};
use rusqlite::params;

use crate::{
    network::{self, WifiTelemetry},
    watcher::{self, IncidentSummary},
};

const C1: Color = Color::Rgb(142, 197, 255);
const C2: Color = Color::Rgb(43, 127, 255);
const C3: Color = Color::Rgb(21, 93, 252);
const C4: Color = Color::Rgb(20, 71, 230);
const C5: Color = Color::Rgb(25, 60, 184);
const SUCCESS: Color = Color::Rgb(52, 211, 153);
const WARNING: Color = Color::Rgb(251, 191, 36);
const ERROR: Color = Color::Rgb(248, 113, 113);
const MUTED: Color = Color::Rgb(148, 163, 184);

pub struct TuiConfig {
    pub device_id: String,
    pub database_path: String,
    pub host_name: String,
    pub hardware_model: Option<String>,
    pub os: String,
    pub arch: String,
    pub stream_subscription_enabled: bool,
    pub role: Option<String>,
    pub org_name: Option<String>,
    pub email: Option<String>,
}

struct DashboardState {
    sync_state: String,
    incidents: Vec<IncidentSummary>,
    wifi: Option<WifiTelemetry>,
    last_error: Option<String>,
    started_at: Instant,
    last_update_at: Option<Instant>,
}

impl DashboardState {
    fn new() -> Self {
        Self {
            sync_state: "sync_state=idle".to_string(),
            incidents: Vec::new(),
            wifi: network::read_wifi_telemetry(),
            last_error: None,
            started_at: Instant::now(),
            last_update_at: None,
        }
    }

    fn mark_update(&mut self) {
        self.last_update_at = Some(Instant::now());
    }
}

pub async fn run(db: PowerSyncDatabase, config: TuiConfig) -> Result<()> {
    let mut terminal = init_terminal()?;
    let mut state = DashboardState::new();

    let status_stream = db.watch_status();
    let mut status_stream = std::pin::pin!(status_stream);

    let incidents_stream = db.watch_statement(
        "SELECT id, title, status, created_at FROM incidents WHERE severity = 'CRITICAL' OR ai_severity = 'CRITICAL' ORDER BY created_at DESC LIMIT 20".to_string(),
        params![],
        |stmt, params| {
            let mut rows = stmt.query(params)?;
            let mut incidents = Vec::new();

            while let Some(row) = rows.next()? {
                incidents.push(IncidentSummary {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    status: row
                        .get::<_, Option<String>>("status")?
                        .unwrap_or_else(|| "UNKNOWN".to_string()),
                    created_at: row.get("created_at")?,
                });
            }

            Ok(incidents)
        },
    );
    let mut incidents_stream = std::pin::pin!(incidents_stream);

    let mut queue_tick = tokio::time::interval(Duration::from_millis(750));
    let mut draw_tick = tokio::time::interval(Duration::from_millis(100));

    let result = async {
        loop {
            tokio::select! {
                maybe = status_stream.next() => {
                    if let Some(status) = maybe {
                        state.sync_state = watcher::format_sync_status(status.as_ref());
                        if state.sync_state.contains("error=") {
                            state.last_error = Some(state.sync_state.clone());
                        }
                        state.mark_update();
                    }
                }
                maybe = incidents_stream.next() => {
                    if let Some(result) = maybe {
                        match result {
                            Ok(next) => {
                                state.incidents = next;
                                state.mark_update();
                            }
                            Err(error) => {
                                state.last_error = Some(format!("critical incident watcher failed: {error}"));
                                state.mark_update();
                            }
                        }
                    }
                }
                _ = queue_tick.tick() => {
                    state.wifi = network::read_wifi_telemetry();
                    if let Ok(depth) = watcher::read_local_write_queue_depth(&db).await {
                        if depth > 0 {
                            break Err(anyhow::anyhow!(watcher::local_write_guard_message(depth)));
                        }
                    }
                }
                _ = draw_tick.tick() => {
                    terminal.draw(|frame| draw(frame.area(), frame, &state, &config))?;
                    if should_quit_from_input()? {
                        break Ok(());
                    }
                }
                signal = tokio::signal::ctrl_c() => {
                    signal.context("failed to listen for ctrl-c")?;
                    break Ok(());
                }
            }
        }
    }.await;

    restore_terminal(&mut terminal)?;
    result
}

fn should_quit_from_input() -> Result<bool> {
    while event::poll(Duration::from_millis(0)).context("failed to poll terminal events")? {
        let event = event::read().context("failed to read terminal event")?;
        if let Event::Key(key) = event {
            if should_quit(key) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn draw(area: Rect, frame: &mut ratatui::Frame, state: &DashboardState, config: &TuiConfig) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    frame.render_widget(header(), root[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(root[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(7),
        ])
        .split(main[0]);

    frame.render_widget(sync_block(state), left[0]);
    frame.render_widget(queue_block(state), left[1]);
    frame.render_widget(config_block(config), left[2]);
    frame.render_widget(incidents_block(state), main[1]);
    frame.render_widget(footer(state), root[2]);
}

fn header() -> Paragraph<'static> {
    let lines = vec![
        Line::from(vec![
            span(C5, "███████╗"),
            span(C4, "██╗"),
            span(C3, "███████╗"),
            span(C4, "██╗     "),
            span(C3, "██████╗ "),
            span(C2, "███╗   ███╗"),
            span(C1, "██╗"),
            span(C3, "██████╗ "),
        ]),
        Line::from(vec![
            span(C5, "██╔════╝"),
            span(C4, "██║"),
            span(C3, "██╔════╝"),
            span(C4, "██║     "),
            span(C3, "██╔══██╗"),
            span(C2, "████╗ ████║"),
            span(C1, "██║"),
            span(C3, "██╔══██╗"),
        ]),
        Line::from(vec![
            span(C4, "█████╗  "),
            span(C3, "██║"),
            span(C2, "█████╗  "),
            span(C3, "██║     "),
            span(C4, "██║  ██║"),
            span(C2, "██╔████╔██║"),
            span(C1, "██║"),
            span(C4, "██║  ██║"),
        ]),
        Line::from(vec![
            span(C3, "██╔══╝  "),
            span(C4, "██║"),
            span(C4, "██╔══╝  "),
            span(C2, "██║     "),
            span(C3, "██║  ██║"),
            span(C4, "██║╚██╔╝██║"),
            span(C2, "██║"),
            span(C3, "██║  ██║"),
        ]),
        Line::from(vec![
            span(C2, "██║     "),
            span(C3, "██║"),
            span(C4, "███████╗"),
            span(C3, "███████╗"),
            span(C2, "██████╔╝"),
            span(C3, "██║ ╚═╝ ██║"),
            span(C4, "██║"),
            span(C5, "██████╔╝"),
        ]),
        Line::from(vec![
            span(C1, "╚═╝     "),
            span(C2, "╚═╝"),
            span(C3, "╚══════╝"),
            span(C4, "╚══════╝"),
            span(C3, "╚═════╝ "),
            span(C5, "╚═╝     ╚═╝"),
            span(C4, "╚═╝"),
            span(C5, "╚═════╝ "),
        ]),
    ];

    Paragraph::new(Text::from(lines))
        .block(
            Block::bordered()
                .title(" FieldMid Edge Dashboard ")
                .border_style(Style::default().fg(C3)),
        )
        .wrap(Wrap { trim: false })
}

fn sync_block(state: &DashboardState) -> Paragraph<'static> {
    let sync_label = compact_sync_state(&state.sync_state);
    let sync_style = sync_state_style(sync_label);
    let signal_level = state
        .wifi
        .as_ref()
        .map(|wifi| connectivity_level_from_percent(wifi.quality_percent))
        .unwrap_or_else(|| connectivity_level(&state.sync_state));
    let bars = signal_bars(signal_level);
    let signal_color = connectivity_style(signal_level);
    let link_text = if let Some(wifi) = &state.wifi {
        match wifi.signal_dbm {
            Some(dbm) => format!("{} {}% ({} dBm)", wifi.interface, wifi.quality_percent, dbm),
            None => format!("{} {}%", wifi.interface, wifi.quality_percent),
        }
    } else {
        "no wifi telemetry".to_string()
    };

    let body = vec![
        Line::from(vec![
            Span::styled("State  ", Style::default().fg(MUTED)),
            Span::styled(
                sync_label.to_string(),
                sync_style.add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Link   ", Style::default().fg(MUTED)),
            Span::styled(bars, signal_color.add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Radio  ", Style::default().fg(MUTED)),
            Span::styled(link_text, Style::default().fg(C1)),
        ]),
        Line::from(vec![
            Span::styled("Errors ", Style::default().fg(MUTED)),
            Span::styled(
                if state.last_error.is_some() {
                    "present"
                } else {
                    "none"
                },
                if state.last_error.is_some() {
                    Style::default().fg(ERROR).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(SUCCESS)
                },
            ),
        ]),
    ];

    Paragraph::new(Text::from(body))
        .block(
            Block::bordered()
                .title(" Sync ")
                .border_style(Style::default().fg(C2)),
        )
        .wrap(Wrap { trim: true })
}

fn queue_block(state: &DashboardState) -> Paragraph<'static> {
    let body = vec![Line::from(vec![
        Span::styled("Critical rows  ", Style::default().fg(MUTED)),
        Span::styled(
            state.incidents.len().to_string(),
            Style::default().fg(C1).add_modifier(Modifier::BOLD),
        ),
    ])];

    Paragraph::new(Text::from(body))
        .block(
            Block::bordered()
                .title(" Incidents ")
                .border_style(Style::default().fg(C4)),
        )
        .wrap(Wrap { trim: true })
}

fn config_block(config: &TuiConfig) -> Paragraph<'static> {
    let mut lines = vec![];

    if let Some(email) = &config.email {
        lines.push(Line::from(vec![
            Span::styled("User    ", Style::default().fg(MUTED)),
            Span::styled(email.clone(), Style::default().fg(Color::White)),
        ]));
    }

    if let Some(role) = &config.role {
        let role_color = match role.as_str() {
            "admin" => C3,
            "supervisor" => WARNING,
            _ => MUTED,
        };
        lines.push(Line::from(vec![
            Span::styled("Role    ", Style::default().fg(MUTED)),
            Span::styled(
                role.clone(),
                Style::default().fg(role_color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    if let Some(org) = &config.org_name {
        lines.push(Line::from(vec![
            Span::styled("Org     ", Style::default().fg(MUTED)),
            Span::styled(org.clone(), Style::default().fg(C1).add_modifier(Modifier::BOLD)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Device  ", Style::default().fg(MUTED)),
        Span::styled(config.device_id.clone(), Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Host    ", Style::default().fg(MUTED)),
        Span::styled(
            if let Some(model) = &config.hardware_model {
                format!("{} ({model})", config.host_name)
            } else {
                config.host_name.clone()
            },
            Style::default().fg(Color::White),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("System  ", Style::default().fg(MUTED)),
        Span::styled(
            format!("{} / {}", config.os, config.arch),
            Style::default().fg(C1),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("DB      ", Style::default().fg(MUTED)),
        Span::styled(shorten_middle(&config.database_path, 34), Style::default().fg(C5)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Mode    ", Style::default().fg(MUTED)),
        Span::styled(
            "read-only",
            Style::default().fg(SUCCESS).add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Stream  ", Style::default().fg(MUTED)),
        Span::styled(
            if config.stream_subscription_enabled {
                "configured"
            } else {
                "default auto-subscribe"
            },
            Style::default().fg(C1),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "Controls: q / Esc / Ctrl+C",
        Style::default().fg(C5).add_modifier(Modifier::DIM),
    )]));

    Paragraph::new(Text::from(lines))
        .block(
            Block::bordered()
                .title(" Runtime ")
                .border_style(Style::default().fg(C5)),
        )
        .wrap(Wrap { trim: true })
}

fn incidents_block(state: &DashboardState) -> List<'static> {
    let items: Vec<ListItem<'static>> = if state.incidents.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No critical incidents in local replica yet.",
            Style::default().fg(MUTED),
        )]))]
    } else {
        state
            .incidents
            .iter()
            .map(|incident| {
                let id_short = short_id(&incident.id);
                let created = incident.created_at.as_deref().unwrap_or("-");
                let title = format!("{} [{}]", incident.title, incident.status);
                let meta = format!("{created}  |  {id_short}");

                ListItem::new(Text::from(vec![
                    Line::from(vec![Span::styled(
                        title,
                        Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(vec![Span::styled(meta, Style::default().fg(MUTED))]),
                ]))
            })
            .collect()
    };

    List::new(items).block(
        Block::bordered()
            .title(" Critical Incident Feed ")
            .border_style(Style::default().fg(C2)),
    )
}

fn footer(state: &DashboardState) -> Paragraph<'static> {
    let uptime = format_duration(state.started_at.elapsed());
    let update_age = state
        .last_update_at
        .map(|instant| format!("{} ago", format_duration(instant.elapsed())))
        .unwrap_or_else(|| "n/a".to_string());

    let status = if let Some(error) = &state.last_error {
        Span::styled(
            format!("Last error: {error}"),
            Style::default().fg(ERROR).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("No watcher errors", Style::default().fg(SUCCESS))
    };

    Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::styled("Uptime ", Style::default().fg(MUTED)),
            Span::styled(uptime, Style::default().fg(C1).add_modifier(Modifier::BOLD)),
            Span::styled("   Last update ", Style::default().fg(MUTED)),
            Span::styled(
                update_age,
                Style::default().fg(C1).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![status]),
    ]))
    .block(
        Block::bordered()
            .title(" Status ")
            .border_style(Style::default().fg(C4)),
    )
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal backend")?;
    terminal.clear().context("failed to clear terminal")?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")?;
    Ok(())
}

fn should_quit(key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }

    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn sync_state_style(value: &str) -> Style {
    if value.contains("error") {
        return Style::default().fg(ERROR);
    }

    if value.contains("uploading") || value.contains("downloading") || value.contains("connecting")
    {
        return Style::default().fg(WARNING);
    }

    if value.contains("connected") {
        return Style::default().fg(SUCCESS);
    }

    Style::default().fg(MUTED)
}

fn compact_sync_state(value: &str) -> &str {
    if value.contains("download_error") {
        "download_error"
    } else if value.contains("upload_error") {
        "upload_error"
    } else if value.contains("uploading") {
        "uploading"
    } else if value.contains("downloading") {
        "downloading"
    } else if value.contains("connected") {
        "connected"
    } else if value.contains("connecting") {
        "connecting"
    } else {
        "idle"
    }
}

fn connectivity_level(value: &str) -> u8 {
    if value.contains("download_error") || value.contains("upload_error") {
        return 0;
    }

    if value.contains("connecting") {
        return 1;
    }

    if value.contains("uploading") || value.contains("downloading") {
        return 3;
    }

    if value.contains("connected") {
        return 4;
    }

    2
}

fn connectivity_level_from_percent(percent: u8) -> u8 {
    match percent {
        0..=10 => 0,
        11..=35 => 1,
        36..=60 => 2,
        61..=80 => 3,
        _ => 4,
    }
}

fn signal_bars(level: u8) -> String {
    let mut bars = String::with_capacity(7);
    for idx in 0..4 {
        if idx < level {
            bars.push('▮');
        } else {
            bars.push('▯');
        }
        if idx < 3 {
            bars.push(' ');
        }
    }
    bars
}

fn connectivity_style(level: u8) -> Style {
    match level {
        0 => Style::default().fg(ERROR),
        1 | 2 => Style::default().fg(WARNING),
        _ => Style::default().fg(SUCCESS),
    }
}

fn span(color: Color, text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

fn shorten_middle(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    let keep = max_len.saturating_sub(3) / 2;
    let start: String = value.chars().take(keep).collect();
    let end: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    format!("{start}...{end}")
}
