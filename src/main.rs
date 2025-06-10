use clap::{Arg, Command};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
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

// helper function to extract inning number from text
fn extract_inning_number(text: &str) -> Option<u32> {
    let ordinals = [ 
        ("1st", 1), ("2nd", 2), ("3rd", 3), ("4th", 4), ("5th", 5), 
        ("6th", 6), ("7th", 7), ("8th", 8), ("9th", 9), ("10th", 10), 
        ("11th", 11), ("12th", 12), ("13th", 13), ("14th", 14), ("15th", 15), 
    ];

    for (ordinal, number) in ordinals.iter() {
        if text.contains(ordinal) {
            return Some(*number);
        }
    }

    // simple pattern matching for digits
    let words: Vec<&str> = text.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        if let Ok(num) = word.parse::<u32>() {
            // check if next word suggests this is an inning
            if i + 1 < words.len() {
                let next_word = words[i + 1].to_lowercase();
                if next_word.contains("inning") || next_word.contains("inn") {
                    return Some(num);
                }
            }

            // or if it's preceded by inning-esque words
            if i > 0 {
                let prev_word = words[i - 1].to_lowercase();
                if prev_word.contains("inning") || prev_word.contains("inn") {
                    return Some(num);
                }
            }
        }
    }

    None
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

    async fn fetch_data(&mut self) -> Result<(), Box<dyn Error>> {
        self.is_refreshing = true;
        
        let sport_code = match self.selected_league.to_lowercase().as_str() {
            "mlb" => "baseball/mlb",
            "nba" => "basketball/nba",
            "wnba" => "basketball/wnba", 
            "nfl" => "football/nfl",
            "nhl" => "hockey/nhl",
            "mls" => "soccer/usa.1",
            "nwsl" => "soccer/usa.nwsl",
            "premier" | "epl" | "prem" | "premier-league" => "soccer/eng.1",
            _ => return Err(format!("Unsupported league: {}", self.selected_league).into()),
        };

        let url = format!("https://site.api.espn.com/apis/site/v2/sports/{}/scoreboard", sport_code);
        
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", "scrbrd/0.1.0")
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

    fn get_scrollable_lines(&self) -> Vec<String> {
        let filtered_events = self.get_filtered_events();
        let mut all_lines = Vec::new();
        
        for event in filtered_events {
            let game_lines = self.format_game_block(event);
            for line in game_lines {
                all_lines.push(line);
            }
            all_lines.push("".to_string()); // spacer between games
        }
        
        all_lines
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, max_visible_lines: usize) {
        let total_lines = self.get_scrollable_lines().len();
        if self.scroll_offset + max_visible_lines < total_lines {
            self.scroll_offset += 1;
        }
    }

    fn format_game_block(&self, event: &GameEvent) -> Vec<String> {
        let mut lines = Vec::new();
        
        for competition in &event.competitions {
            if competition.competitors.len() >= 2 {
                let away = &competition.competitors[0];
                let home = &competition.competitors[1];
                
                // main score line - centered
                let score_line = format!(
                    "{} {} - {} {}",
                    away.team.abbreviation,
                    away.score,
                    home.score,
                    home.team.abbreviation
                );
                lines.push(score_line);
                lines.push("".to_string()); // line break after score

                // status/time line - centered
                let status_line = self.format_status(&competition.status);
                if !status_line.is_empty() {
                    lines.push(status_line);
                }

                // records line - centered
                if !away.records.is_empty() || !home.records.is_empty() {
                    let away_record = away.records.first()
                        .map(|r| r.summary.clone())
                        .unwrap_or_default();
                    let home_record = home.records.first()
                        .map(|r| r.summary.clone())
                        .unwrap_or_default();
                    
                    if !away_record.is_empty() || !home_record.is_empty() {
                        lines.push(format!("({}) vs ({})", away_record, home_record));
                        lines.push("".to_string()); // line break after records
                    }
                }
            }
        }
        
        lines
    }

    fn format_status(&self, status: &Status) -> String {
        match status.status_type.state.as_str() {
            "pre" => {
                status.status_type.short_detail.clone()
            },
            "in" => {
                let period_name = match self.selected_league.to_lowercase().as_str() {
                    "nfl" | "football" => match status.period {
                        1..=4 => format!("Q{}", status.period),
                        5 => "OT".to_string(),
                        _ => format!("Q{}", status.period),
                    },
                    "nba" | "wnba" | "basketball" => match status.period {
                        1..=4 => format!("Q{}", status.period),
                        5.. => "OT".to_string(),
                        _ => format!("Q{}", status.period),
                    },
                    "nhl" | "hockey" => match status.period {
                        1..=3 => format!("P{}", status.period),
                        4.. => "OT".to_string(),
                        _ => format!("P{}", status.period),
                    },
                    "mlb" | "baseball" => {
                        // try to parse inning info
                        if let Some(inning_info) = self.parse_baseball_inning(status) {
                            inning_info
                        } else if status.period <= 9 {
                            // fallback: try to determine from short_detail or detail
                            if status.status_type.short_detail.to_lowercase().contains("bot") ||
                                status.status_type.detail.to_lowercase().contains("bot") ||
                                status.status_type.short_detail.to_lowercase().contains("bottom") {
                                    format!("B{}", status.period)
                            } else if status.status_type.short_detail.to_lowercase().contains("top") ||
                                    status.status_type.detail.to_lowercase().contains("top") {
                                    format!("T{}", status.period)
                            } else {
                                format!("{}", status.period)
                            }
                        } else {
                            format!("{}", status.period)
                        }
                    },
                    "mls" | "nwsl" | "premier" | "epl" | "soccer" => {
                        if status.period == 1 {
                            format!("{}' 1H", status.display_clock)
                        } else if status.period == 2 {
                            format!("{}' 2H", status.display_clock)
                        } else {
                            format!("{}' ET", status.display_clock)
                        }
                    },
                    _ => format!("{} - {}", status.period, status.display_clock),
                };

                format!("live - {}", period_name)
            },
            "post" => {
                if status.status_type.completed {
                    "final".to_string()
                } else {
                    status.status_type.short_detail.clone()
                }
            },
            _ => status.status_type.short_detail.clone(),
        }
    }

    // helper function to parse inning information
    fn parse_baseball_inning(&self, status: &Status) -> Option<String> {
        // check various fields for inning info
        let sources = vec![
            &status.status_type.short_detail,
            &status.status_type.detail,
            &status.status_type.description,
            &status.display_clock,
        ];

        for source in sources {
            let source_lower = source.to_lowercase();

            // look for patterns like "top 3rd," "bot 5th," etc.
            if source_lower.contains("top") {
                if let Some(inning) = extract_inning_number(&source_lower) {
                    return Some(format!("T{}", inning));
                }
            } else if source_lower.contains("bot") || source_lower.contains("bottom") {
                if let Some(inning) = extract_inning_number(&source_lower) {
                    return Some(format!("B{}", inning));
                }
            }
            
            // look for "mid 3rd" or similar
            if source_lower.contains("mid") {
                if let Some(inning) = extract_inning_number(&source_lower) {
                    return Some(format!("M{}", inning));
                }
            }
        }

        None
    }
}

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
            let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(3),
            ].as_ref())
            .split(f.size());

            // calculate these variables at the top so they're in scope for the entire function
            let filtered_events = app.get_filtered_events();
            let content_width = chunks[1].width.saturating_sub(2) as usize; // account for borders
            let content_height = chunks[1].height.saturating_sub(2) as usize; // account for borders
            let can_fit_two_columns = content_width >= 80; // minimum width for two columns

            // header with refresh indicator
            let refresh_indicator = if app.is_refreshing {
            " ↻"
            } else {
                ""
    };
        
    let title = match &app.team_filter {
        Some(team) => format!("{} - {}{}", app.selected_league.to_uppercase(), team.to_uppercase(), refresh_indicator),
        None => format!("{} scrbrd{}", app.selected_league.to_lowercase(), refresh_indicator),
    };
        
    let header = Paragraph::new(title)
    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    .alignment(Alignment::Center) 
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // main content
    if let Some(ref error) = app.error_message {
        let error_msg = Paragraph::new(format!("error: {}", error))
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
            f.render_widget(error_msg, chunks[1]);
        } else {
            if filtered_events.is_empty() {
                let no_games = Paragraph::new("no games found :c")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
                f.render_widget(no_games, chunks[1]);
            } else {
                // check if we need scrolling
                let all_lines = app.get_scrollable_lines();
                let needs_scrolling = all_lines.len() > content_height;
                
                if can_fit_two_columns && filtered_events.len() > 1 {
                    // two-column layout with scrolling support
                    let game_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(chunks[1]);
                    
                    // get all events for scrolling
                    let start_event = if needs_scrolling {
                        (app.scroll_offset / 4).min(filtered_events.len().saturating_sub(1))
                    } else {
                        0
                    };
                    
                    let visible_events = if needs_scrolling {
                        let events_per_screen = (content_height / 4).max(1); // roughly 4 lines per game
                        let end_event = (start_event + events_per_screen * 2).min(filtered_events.len());
                        &filtered_events[start_event..end_event]
                    } else {
                        &filtered_events[..]
                    };
                    
                    // left column
                    let left_events: Vec<_> = visible_events.iter()
                        .enumerate()
                        .filter(|(i, _)| i % 2 == 0)
                        .map(|(_, event)| *event)
                        .collect();
                    
                    let mut left_lines = Vec::new();
                    for event in left_events {
                        let game_lines = app.format_game_block(event);
                        for line in game_lines {
                            left_lines.push(line);
                        }
                        left_lines.push("".to_string()); // spacer between games
                    }
                    
                    let left_items: Vec<ListItem> = left_lines.iter()
                        .map(|line| {
                            let style = get_line_style(line);
                            ListItem::new(Line::from(vec![
                                Span::styled(line.clone(), style),
                            ]).alignment(Alignment::Center))
                        })
                        .collect();

                    let left_title = if needs_scrolling {
                        format!(" games ({}/{}) ", start_event + 1, filtered_events.len())
                    } else {
                        " games ".to_string()
                    };

                    let left_list = List::new(left_items)
                        .block(Block::default().borders(Borders::RIGHT).title(left_title))
                        .style(Style::default().fg(Color::White));
                    f.render_widget(left_list, game_chunks[0]);

                    // right column
                    let right_events: Vec<_> = visible_events.iter()
                        .enumerate()
                        .filter(|(i, _)| i % 2 == 1)
                        .map(|(_, event)| *event)
                        .collect();
                    
                    let mut right_lines = Vec::new();
                    for event in right_events {
                        let game_lines = app.format_game_block(event);
                        for line in game_lines {
                            right_lines.push(line);
                        }
                        right_lines.push("".to_string()); // spacer between games
                    }
                    
                    let right_items: Vec<ListItem> = right_lines.iter()
                        .map(|line| {
                            let style = get_line_style(line);
                            ListItem::new(Line::from(vec![
                                Span::styled(line.clone(), style),
                            ]).alignment(Alignment::Center))
                        })
                        .collect();

                    let right_list = List::new(right_items)
                        .block(Block::default().borders(Borders::LEFT))
                        .style(Style::default().fg(Color::White));
                    f.render_widget(right_list, game_chunks[1]);
                    
                } else {
                    // single column layout (centered) with scrolling support
                    let total_lines = all_lines.len();
                    let visible_lines = if needs_scrolling {
                        let start = app.scroll_offset;
                        let end = (start + content_height).min(total_lines);
                        all_lines[start..end].to_vec()
                    } else {
                        all_lines
                    };

                    let game_items: Vec<ListItem> = visible_lines.iter()
                        .map(|line| {
                            let style = get_line_style(line);
                            ListItem::new(Line::from(vec![
                                Span::styled(line.clone(), style),
                            ]).alignment(Alignment::Center))
                        })
                        .collect();

                    let title = if needs_scrolling {
                        format!(" games ({}/{}) ", 
                            app.scroll_offset + 1, 
                            total_lines.saturating_sub(content_height).max(1)
                        )
                    } else {
                        " games ".to_string()
                    };

                    let games_list = List::new(game_items)
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .style(Style::default().fg(Color::White));
                    f.render_widget(games_list, chunks[1]);
                }
            }
        }

        // footer with refresh timer
        let all_lines_count = app.get_scrollable_lines().len();
        let show_scroll_help = all_lines_count > content_height || 
                             (can_fit_two_columns && filtered_events.len() > (content_height / 4) * 2);
        
        let time_left = app.time_until_next_refresh().as_secs();
        let scroll_text = if show_scroll_help { " | ↑/↓ - scroll" } else { "" };
        let footer_text = format!("q - quit | r - refresh{} | next: {}s", scroll_text, time_left);
        
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    })?;

    // handle input with timeout for refresh checking
    if event::poll(Duration::from_millis(500))? {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('r') => {
                    // manual refresh
                    if let Err(e) = app.fetch_data().await {
                        app.error_message = Some(format!("refresh failed: {}", e));
                    }
                }
                KeyCode::Up => {
                    app.scroll_up();
                }
                KeyCode::Down => {
                    let content_height = terminal.size()?.height.saturating_sub(8); // account for header/footer
                    app.scroll_down(content_height as usize);
                }
                _ => {}
            }
        }
    }
}

// cleanup
disable_raw_mode()?;
execute!(
    terminal.backend_mut(),
    LeaveAlternateScreen,
    DisableMouseCapture
)?;
terminal.show_cursor()?;

Ok(())
}

fn get_line_style(line: &str) -> Style {
if line.contains("live") {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
} else if line.contains("final") {
    Style::default().fg(Color::Green)
} else if line.is_empty() {
    Style::default()
} else {
    Style::default().fg(Color::White)
}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
let matches = Command::new("scrbrd")
    .version("0.1.0")
    .author("Chuck Swung")
    .about("A minimal terminal sports scoreboard using ESPN API")
    .arg(
        Arg::new("league")
            .short('l')
            .long("league")
            .value_name("LEAGUE")
            .help("League: mlb, nba, wnba, nfl, nhl, mls, nwsl, premier")
            .required(true)
    )
    .arg(
        Arg::new("team")
            .short('t')
            .long("team")
            .value_name("TEAM")
            .help("Filter by team name")
    )
    .get_matches();

let league = matches.get_one::<String>("league").unwrap().to_string();
let team = matches.get_one::<String>("team").map(|s| s.to_string());

let mut app = AppState::new(league, team);

// fetch initial data
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