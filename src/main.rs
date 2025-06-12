use clap::{Arg, Command};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use serde::{Deserialize, Serialize};
use tokio;


// data models


#[derive(Debug, Clone, Serialize, Deserialize)]
struct Competition {
    id: String,
    date: String,
    competitors: Vec<Competitor>,
    status: Status,
    #[serde(default)]
    broadcasts: Vec<Broadcast>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Competitor {
    team: Team,
    score: String,
    #[serde(rename = "homeAway")]
    home_away: String,
    #[serde(default)]
    records: Vec<Record>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Team {
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "shortDisplayName")]
    short_display_name: String,
    abbreviation: String,
    color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Status {
    #[serde(rename = "type")]
    status_type: StatusType,
    #[serde(rename = "displayClock")]
    display_clock: String,
    period: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusType {
    name: String,
    state: String,
    completed: bool,
    description: String,
    detail: String,
    #[serde(rename = "shortDetail")]
    short_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
    name: String,
    summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Broadcast {
    names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EspnResponse {
    events: Vec<GameEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameEvent {
    id: String,
    name: String,
    #[serde(rename = "shortName")]
    short_name: String,
    date: String,
    competitions: Vec<Competition>,
}

// app state

#[derive(Debug, Clone)]
struct AppState {
    events: Vec<GameEvent>,
    selected_league: String,
    team_filter: Option<String>,
    error_message: Option<String>,
    scroll_offset: usize,
    last_refresh: Instant,
    is_refreshing: bool,
}

impl AppState {
    fn new(league: String, team: Option<String>) -> Self {
        Self {
            events: Vec::new(),
            selected_league: league,
            team_filter: team,
            error_message: None,
            scroll_offset: 0,
            last_refresh: Instant::now(),
            is_refreshing: false,
        }
    }

    fn get_filtered_events(&self) -> Vec<&GameEvent> {
        if let Some(ref filter) = self.team_filter {
            let filter_lower = filter.to_lowercase();
            self.events.iter()
                .filter(|event| {
                    event.competitions.iter().any(|comp| {
                        comp.competitors.iter().any(|competitor| {
                            competitor.team.display_name.to_lowercase().contains(&filter_lower) ||
                            competitor.team.short_display_name.to_lowercase().contains(&filter_lower) ||
                            competitor.team.abbreviation.to_lowercase().contains(&filter_lower)
                        })
                    })
                })
                .collect()
        } else {
            self.events.iter().collect()
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        let total_games = self.get_filtered_events().len();
        if self.scroll_offset + 1 < total_games {
            self.scroll_offset += 1;
        }
    }
}

// data fetching

impl AppState {
    async fn fetch_data(&mut self) -> Result<(), Box<dyn Error>> {
        self.is_refreshing = true;
        
        let sport_code = get_sport_code(&self.selected_league)?;
        let url = format!("https://site.api.espn.com/apis/site/v2/sports/{}/scoreboard", sport_code);
        
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "scrbrd/0.2.0")
            .send()
            .await?;

        if !response.status().is_success() {
            self.is_refreshing = false;
            return Err(format!("ESPN API error: {}", response.status()).into());
        }

        let espn_data: EspnResponse = response.json().await?;
        self.events = espn_data.events;
        self.error_message = None;
        self.last_refresh = Instant::now();
        self.is_refreshing = false;
        
        Ok(())
    }

    fn should_refresh(&self) -> bool {
        self.last_refresh.elapsed() >= Duration::from_secs(30)
    }

    fn time_until_next_refresh(&self) -> Duration {
        let elapsed = self.last_refresh.elapsed();
        if elapsed >= Duration::from_secs(30) {
            Duration::from_secs(0)
        } else {
            Duration::from_secs(30) - elapsed
        }
    }
}

fn get_sport_code(league: &str) -> Result<&'static str, Box<dyn Error>> {
    match league.to_lowercase().as_str() {
        "mlb" => Ok("baseball/mlb"),
        "nba" => Ok("basketball/nba"),
        "wnba" => Ok("basketball/wnba"), 
        "nfl" => Ok("football/nfl"),
        "nhl" => Ok("hockey/nhl"),
        "mls" => Ok("soccer/usa.1"),
        "nwsl" => Ok("soccer/usa.nwsl"),
        "premier" | "epl" | "prem" | "premier-league" => Ok("soccer/eng.1"),
        _ => Err(format!("unsupported league: {}", league).into()),
    }
}

// score block formatting

impl AppState {
    fn format_game_widget(&self, event: &GameEvent) -> Paragraph {
        let mut content = Vec::new();

        for competition in &event.competitions {
            if competition.competitors.len() >= 2 {
                let away = &competition.competitors[0];
                let home = &competition.competitors[1];
                
                // score line
                let score_line = format!(
                    "{} {} - {} {}",
                    away.team.abbreviation,
                    away.score,
                    home.score,
                    home.team.abbreviation
                );
                
                let score_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
                content.push(Line::from(vec![
                    Span::styled(score_line, score_style)
                ]).alignment(Alignment::Center));
                
                // status line
                let status_line = self.format_status(&competition.status);
                if !status_line.is_empty() {
                    let status_style = get_status_style(&status_line);
                    content.push(Line::from(vec![
                        Span::styled(status_line, status_style)
                    ]).alignment(Alignment::Center));
                    content.push(Line::from(""));
                }

                // records line
                add_records_line(&mut content, away, home);
            }
        }

        Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL))
            .alignment(Alignment::Center)
    }

    fn format_status(&self, status: &Status) -> String {
        match status.status_type.state.as_str() {
            "pre" => status.status_type.short_detail.clone(),
            "in" => format!("🔴 LIVE | {}", self.format_live_status(status)),
            "post" => {
                if status.status_type.completed {
                    "FINAL".to_string()
                } else {
                    status.status_type.short_detail.clone()
                }
            },
            _ => status.status_type.short_detail.clone(),
        }
    }

    fn format_live_status(&self, status: &Status) -> String {
        match self.selected_league.to_lowercase().as_str() {
            "nfl" | "football" => format_football_status(status),
            "nba" | "wnba" | "basketball" => format_basketball_status(status),
            "nhl" | "hockey" => format_hockey_status(status),
            "mlb" | "baseball" => format_baseball_status(status),
            "mls" | "nwsl" | "premier" | "epl" | "soccer" => format_soccer_status(status),
            _ => format!("{} - {}", status.period, status.display_clock),
        }
    }
}

fn add_records_line(content: &mut Vec<Line>, away: &Competitor, home: &Competitor) {
    if !away.records.is_empty() || !home.records.is_empty() {
        let away_record = away.records.first()
            .map(|r| r.summary.clone())
            .unwrap_or_default();
        let home_record = home.records.first()
            .map(|r| r.summary.clone())
            .unwrap_or_default();
        
        if !away_record.is_empty() || !home_record.is_empty() {
            let record_line = format!("({}) vs ({})", away_record, home_record);
            content.push(Line::from(vec![
                Span::styled(record_line, Style::default().fg(Color::Gray))
            ]).alignment(Alignment::Center));
        }
    }
}

fn get_status_style(status: &str) -> Style {
    if status.contains("LIVE") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if status.contains("FINAL") {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

// sport-specific formatting

fn format_football_status(status: &Status) -> String {
    match status.period {
        1..=4 => format!("Q{}", status.period),
        5 => "OT".to_string(),
        _ => format!("Q{}", status.period),
    }
}

fn format_basketball_status(status: &Status) -> String {
    match status.period {
        1..=4 => format!("Q{}", status.period),
        5.. => "OT".to_string(),
        _ => format!("Q{}", status.period),
    }
}

fn format_hockey_status(status: &Status) -> String {
    match status.period {
        1..=3 => format!("P{}", status.period),
        4.. => "OT".to_string(),
        _ => format!("P{}", status.period),
    }
}

fn format_baseball_status(status: &Status) -> String {
    // try to determine inning half from short_detail or detail
    if status.status_type.short_detail.to_lowercase().contains("bot") ||
       status.status_type.detail.to_lowercase().contains("bot") ||
       status.status_type.short_detail.to_lowercase().contains("bottom") ||
       status.status_type.detail.to_lowercase().contains("bottom") {
        return format!("B{}", status.period);
    } else if status.status_type.short_detail.to_lowercase().contains("top") ||
              status.status_type.detail.to_lowercase().contains("top") {
        return format!("T{}", status.period);
    } else if status.status_type.short_detail.to_lowercase().contains("mid") ||
              status.status_type.detail.to_lowercase().contains("mid") ||
              status.status_type.short_detail.to_lowercase().contains("middle") ||
              status.status_type.detail.to_lowercase().contains("middle") {
        return format!("M{}", status.period);
    } else if status.status_type.short_detail.to_lowercase().contains("end") ||
              status.status_type.detail.to_lowercase().contains("end") {
        return format!("E{}", status.period);
    } else {
        // fallback: just return the inning number
        return format!("{}", status.period);
    }
}

fn format_soccer_status(status: &Status) -> String {
    if status.period == 1 {
        format!("{}' 1H", status.display_clock)
    } else if status.period == 2 {
        format!("{}' 2H", status.display_clock)
    } else {
        format!("{}' ET", status.display_clock)
    }
}

// ui layout

impl AppState {
    fn calculate_games_per_screen(&self, content_width: u16, content_height: u16) -> usize {
        let can_fit_two_columns = content_width >= 80;
        let games_per_column = (content_height / 6).max(1) as usize; // roughly 6 lines per boxed game
        
        if can_fit_two_columns {
            games_per_column * 2
        } else {
            games_per_column
        }
    }
}

// ui render

async fn render_scoreboard(app: &mut AppState) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        // check if we need to auto-refresh
        if app.should_refresh() && !app.is_refreshing {
            if let Err(e) = app.fetch_data().await {
                app.error_message = Some(format!("refresh failed: {}", e));
            }
        }

        terminal.draw(|f| {
            let chunks = create_main_layout(f.area());
            let filtered_events = app.get_filtered_events();
            let content_width = chunks[1].width;
            let content_height = chunks[1].height;
            let total_games_per_screen = app.calculate_games_per_screen(content_width, content_height);

            // render header
            render_header(f, &chunks[0], app);

            // render main content
            render_main_content(f, &chunks[1], app, &filtered_events, content_width, total_games_per_screen);

            // render footer
            render_footer(f, &chunks[2], app, &filtered_events, total_games_per_screen);
        })?;

        // handle input with timeout for refresh checking
        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(key) = event::read()? {
                if handle_input(key.code, app).await? {
                    break; // exit requested
                }
            }
        }
    }
    
    cleanup_terminal(&mut terminal)?;
    Ok(())
}

fn create_main_layout(area: ratatui::layout::Rect) -> Vec<ratatui::layout::Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ].as_ref())
        .split(area).to_vec()
}

fn render_header(f: &mut ratatui::Frame, area: &ratatui::layout::Rect, app: &AppState) {
    let title = match &app.team_filter {
        Some(team) => format!("scrbrd | {}", team.to_lowercase()),
        None => format!("scrbrd | {}", app.selected_league.to_lowercase()),
    };
    
    let header = Paragraph::new(title)
        .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center) 
        .block(Block::default());
    f.render_widget(header, *area);
}

fn render_main_content(
    f: &mut ratatui::Frame, 
    area: &ratatui::layout::Rect, 
    app: &AppState, 
    filtered_events: &[&GameEvent],
    content_width: u16,
    total_games_per_screen: usize
) {
    if let Some(ref error) = app.error_message {
        let error_msg = Paragraph::new(format!("error: {}", error))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .block(Block::default());
        f.render_widget(error_msg, *area);
    } else if filtered_events.is_empty() {
        let no_games = Paragraph::new("no games found :c")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default());
        f.render_widget(no_games, *area);
    } else {
        render_games(f, area, app, filtered_events, content_width, total_games_per_screen);
    }
}

fn render_games(
    f: &mut ratatui::Frame,
    area: &ratatui::layout::Rect,
    app: &AppState,
    filtered_events: &[&GameEvent],
    content_width: u16,
    total_games_per_screen: usize
) {
    let start_game = app.scroll_offset;
    let end_game = (start_game + total_games_per_screen).min(filtered_events.len());
    let visible_events = &filtered_events[start_game..end_game];
    let can_fit_two_columns = content_width >= 80;

    if can_fit_two_columns && visible_events.len() > 1 {
        render_two_column_layout(f, area, app, visible_events);
    } else {
        render_single_column_layout(f, area, app, visible_events);
    }
}

fn render_two_column_layout(
    f: &mut ratatui::Frame,
    area: &ratatui::layout::Rect,
    app: &AppState,
    visible_events: &[&GameEvent]
) {
    let game_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(*area);
    
    // split games between columns
    let left_events: Vec<_> = visible_events.iter()
        .enumerate()
        .filter(|(i, _)| i % 2 == 0)
        .map(|(_, event)| *event)
        .collect();
    
    let right_events: Vec<_> = visible_events.iter()
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, event)| *event)
        .collect();
    
    render_column(f, &game_chunks[0], app, &left_events);
    render_column(f, &game_chunks[1], app, &right_events);
}

fn render_single_column_layout(
    f: &mut ratatui::Frame,
    area: &ratatui::layout::Rect,
    app: &AppState,
    visible_events: &[&GameEvent]
) {
    render_column(f, area, app, visible_events);
}

fn render_column(
    f: &mut ratatui::Frame,
    area: &ratatui::layout::Rect,
    app: &AppState,
    events: &[&GameEvent]
) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            events.iter()
                .map(|_| Constraint::Length(6))
                .chain(std::iter::once(Constraint::Min(0)))
                .collect::<Vec<_>>()
        )
        .split(*area);
    
    for (i, event) in events.iter().enumerate() {
        let game_widget = app.format_game_widget(event);
        f.render_widget(game_widget, layout[i]);
    }
}

fn render_footer(
    f: &mut ratatui::Frame,
    area: &ratatui::layout::Rect,
    app: &AppState,
    filtered_events: &[&GameEvent],
    total_games_per_screen: usize
) {
    let needs_scroll = filtered_events.len() > total_games_per_screen;
    let time_left = app.time_until_next_refresh().as_secs();
    let scroll_text = if needs_scroll { "↑ ↓ scroll | " } else { "" };
    let footer_text = format!("q: quit | {}↻ {}", scroll_text, time_left);
    
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default());
    f.render_widget(footer, *area);
}

async fn handle_input(key_code: KeyCode, app: &mut AppState) -> Result<bool, Box<dyn Error>> {
    match key_code {
        KeyCode::Char('q') => Ok(true), // exit
        KeyCode::Char('r') => {
            // manual refresh
            if let Err(e) = app.fetch_data().await {
                app.error_message = Some(format!("refresh failed: {}", e));
            }
            Ok(false)
        }
        KeyCode::Up => {
            app.scroll_up();
            Ok(false)
        }
        KeyCode::Down => {
            app.scroll_down();
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}


// main


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("scrbrd")
        .version("0.2.0")
        .author("Chuck Swung")
        .about("a tui sports tracker for real-time scores and status")
        .arg(
            Arg::new("league")
                .short('l')
                .long("league")
                .value_name("LEAGUE")
                .help("supported leagues: mlb, nba, wnba, nfl, nhl, mls, nwsl, premier")
                .required(true)
        )
        .arg(
            Arg::new("team")
                .short('t')
                .long("team")
                .value_name("TEAM")
                .help("filter by team name, without city (i.e. guardians)")
        )
        .get_matches();

    let league = matches.get_one::<String>("league").unwrap().to_string();
    let team = matches.get_one::<String>("team").map(|s| s.to_string());

    let mut app = AppState::new(league, team);

    // retch initial data
    match app.fetch_data().await {
        Ok(()) => {},
        Err(e) => {
            app.error_message = Some(e.to_string());
        }
    }

    // render the UI with auto-refresh
    render_scoreboard(&mut app).await?;

    Ok(())
}