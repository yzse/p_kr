// New UI module to handle the Terminal UI rendering logic

use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Style, Modifier, Color},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
// Removed unused import Round
use crate::util::get_player_position;

// Render the application UI
pub fn render_ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create horizontal split first for main area and sidebar
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(75),   // Main game area (75% of width)
            Constraint::Percentage(25),   // Right sidebar (25% of width)
        ].as_ref())
        .split(f.size());
        
    // Split the main area vertically
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),   // Reduced game info (status info only)
            Constraint::Length(3),   // Community cards
            Constraint::Length(3),   // Player hand
            Constraint::Min(10),     // Messages (expanded)
            Constraint::Length(3),   // Input
        ].as_ref())
        .split(horizontal_chunks[0]);
        
    // Create sidebar for info
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),  // Game status area (now on top)
            Constraint::Min(12),     // Stack/pot info with visualizations (now below)            
        ].as_ref())
        .split(horizontal_chunks[1]);
    
    // Game info widget (top area with core game status)
    render_game_info(f, app, main_chunks[0]);
    
    // Right sidebar game status area (now on top)
    render_game_status(f, app, sidebar_chunks[0]);
    
    // Right sidebar with chips/pot visualization (now below)
    render_chip_info(f, app, sidebar_chunks[1]);
    
    // Community cards widget
    render_community_cards(f, app, main_chunks[1]);
    
    // Player's hand widget
    render_player_hand(f, app, main_chunks[2]);
    
    // Messages widget (with scrolling)
    render_messages(f, app, main_chunks[3]);
    
    // Input widget
    render_input(f, app, main_chunks[4]);
}

// Render the game info section - now simplified with player status only
fn render_game_info<B: Backend>(f: &mut Frame<B>, app: &mut App, area: tui::layout::Rect) {
    // Show whose turn it is - keep brief for small screens 
    let current_player = &app.game.players[app.game.current_player_idx];
    let current_player_name = &current_player.name;
    let turn_info = if !app.game_active {
        "Press 'd' to deal, 'q' to quit"
    } else if !current_player.is_bot {
        "Your turn."
    } else {
        &format!("Waiting for {}", current_player_name)
    };
    
    // Build player turn indicators - shorter format with clear bot numbering
    let mut player_status = String::new();
    let max_players_to_show = if f.size().width < 80 { 5 } else { app.game.players.len() };
    
    // Find the human player index
    let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
    
    // Track bot number separately from player index
    let mut bot_num = 1;
    
    for (idx, player) in app.game.players.iter().enumerate().take(max_players_to_show) {
        // Determine player status indicator
        let status = if idx == app.game.current_player_idx {
            "➤"   // Current turn
        } else if player.folded {
            "✘"   // Folded
        } else {
            "·"   // Waiting
        };
        
        // Create player display name
        let display_name = if idx == human_idx {
            "You".to_string()
        } else {
            // Use consistent bot numbering (B1, B2, etc.)
            let name = format!("B{}", bot_num);
            bot_num += 1;
            name
        };
        
        player_status.push_str(&format!("{}{} ", display_name, status));
    }
    
    // Indicate if more players aren't shown
    if app.game.players.len() > max_players_to_show {
        player_status.push_str(&format!("(+{})", app.game.players.len() - max_players_to_show));
    }
    
    // Game info
    let human_position = get_player_position(&app.game, human_idx);
    
    let active_players = app.game.players.iter().filter(|p| !p.folded).count();
    
    // Get round result display
    let result_display = if let Some((winner_name, profit)) = &app.round_results {
        let profit_str = if *profit >= 0 {
            format!(" +${}", profit)
        } else {
            format!(" -${}", profit.abs())
        };
        format!("Last hand: {} won.{}", winner_name, profit_str)
    } else {
        "".to_string()
    };
    
    // Game status/controls display - simplified for top area
    let game_status = if app.game_active {
        "Game in progress [s: stop game]"
    } else {
        "Game not active [d: deal new hand, q: quit]"
    };
    
    // Calculate available width to ensure no overflow
    let total_width = area.width as usize - 4; // Account for borders
    let truncate_large = total_width < 70; // If screen is narrow, use shorter format
    
    let game_info = Paragraph::new(vec![
        // Player status (with truncation to prevent overflow)
        Line::from(vec![
            Span::raw("Players: "),
            Span::styled(
                // Ensure player status fits within available width
                if player_status.len() + 10 > total_width {
                    // Safe truncation with bounds checking
                    let safe_len = total_width.saturating_sub(13);
                    if safe_len > 0 && safe_len < player_status.len() {
                        format!("{}..", &player_status[0..safe_len])
                    } else {
                        player_status.chars().take(total_width.saturating_sub(13)).collect::<String>()
                    }
                } else {
                    player_status
                }, 
                Style::default().fg(Color::White))
        ]),
        // Round and position info
        Line::from(vec![
            Span::raw("Round: "),
            Span::styled(format!("{:?}", app.game.round), Style::default().fg(Color::Green)),
            Span::raw(" | Position: "),
            Span::styled(
                // Truncate position name if too long
                if human_position.len() > 15 && truncate_large {
                    format!("{}..", &human_position[0..12])
                } else {
                    human_position
                }, 
                Style::default().fg(Color::Cyan)
            ),
        ]),
        // Game action info (simplified)
        Line::from(vec![
            Span::styled("► ", Style::default().fg(Color::Green)),
            Span::styled(
                if turn_info.len() + 2 > total_width {
                    format!("{}..", &turn_info[0..total_width.saturating_sub(5)])
                } else {
                    turn_info.to_string()
                }, 
                Style::default().fg(Color::Cyan))
        ])
    ])
    .block(Block::default().title("").borders(Borders::ALL));
    f.render_widget(game_info, area);
}

// Render the community cards
// Render the chip/pot/bet visualization sidebar
fn render_chip_info<B: Backend>(f: &mut Frame<B>, app: &App, area: tui::layout::Rect) {
    // Style the pot amount with color based on size
    let pot_style = if app.game.pot > 100 {
        Style::default().fg(Color::Yellow)
    } else if app.game.pot > 50 {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    
    // Get the human player's chip count
    let player_chips = app.game.players.iter()
        .find(|p| !p.is_bot)
        .map(|p| p.chips)
        .unwrap_or(0);
        
    // Get the current highest bet
    let current_bet = app.game.players.iter()
        .map(|p| p.current_bet)
        .max()
        .unwrap_or(0);
    
    // Create a visually interesting display with larger visualizations
    let chip_info = Paragraph::new(vec![
        
        // Pot section with larger visualization
        Line::from(vec![
            Span::raw("POT")
        ]),
        Line::from(vec![
            Span::styled(format!("${}", app.game.pot), 
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(vec![
            Span::styled(
                {
                    let pot = app.game.pot;
                    if pot < 20 {
                        "○"
                    } else if pot < 50 {
                        "○○"
                    } else if pot < 100 {
                        "●●"
                    } else if pot < 200 {
                        "●●●"
                    } else if pot < 400 {
                        "●●●●"
                    } else {
                        "●●●●●"
                    }
                },
                Style::default().fg(if app.game.pot > 200 { Color::Red } 
                    else if app.game.pot > 100 { Color::Yellow } 
                    else { Color::Green })
            )
        ]),
        // Empty line for spacing
        Line::from(vec![Span::raw("")]),
        // Your chips section
        Line::from(vec![
            Span::raw("YOUR CHIPS")
        ]),
        Line::from(vec![
            Span::styled(format!("${}", player_chips), 
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        ]),
        Line::from(vec![
            Span::styled(
                {
                    if player_chips < 30 {
                        "□"
                    } else if player_chips < 70 {
                        "□□"
                    } else if player_chips < 120 {
                        "■■"
                    } else if player_chips < 200 {
                        "■■■"
                    } else if player_chips < 300 {
                        "■■■■"
                    } else {
                        "■■■■■"
                    }
                },
                Style::default().fg(if player_chips < 50 { Color::Red } 
                    else if player_chips < 100 { Color::Yellow } 
                    else { Color::Blue })
            )
        ]),
        // Empty line for spacing
        Line::from(vec![Span::raw("")]),
        // Current bet section
        Line::from(vec![
            Span::raw("CURRENT BET")
        ]),
        Line::from(vec![
            Span::styled(format!("${}", current_bet), 
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        ]),
        Line::from(vec![
            Span::styled(
                {
                    if current_bet == 0 {
                        "-"
                    } else if current_bet < 10 {
                        "▪"
                    } else if current_bet < 30 {
                        "▫▫"
                    } else if current_bet < 60 {
                        "▫▫▫"
                    } else if current_bet < 100 {
                        "▫▫▫▫"
                    } else {
                        "▫▫▫▫▫"
                    }
                },
                Style::default().fg(if current_bet > 70 { Color::Red }
                    else if current_bet > 30 { Color::Yellow }
                    else { Color::Green })
            )
        ])
    ])
    .block(Block::default().title("Stats").borders(Borders::ALL));
    f.render_widget(chip_info, area);
}

// Render game status sidebar
fn render_game_status<B: Backend>(f: &mut Frame<B>, app: &App, area: tui::layout::Rect) {
    // Active players count
    let active_players = app.game.players.iter().filter(|p| !p.folded).count();
    
    // Last game result
    let result_display = if let Some((winner_name, profit)) = &app.round_results {
        let profit_str = if *profit >= 0 {
            format!(" +${}", profit)
        } else {
            format!(" -${}", profit.abs())
        };
        format!("{} won{}", winner_name, profit_str)
    } else {
        "No results yet".to_string()
    };
    
    // Stats
    let stats_display = if !app.game_stats.is_empty() {
        let total_profit = app.game_stats.iter().sum::<i32>();
        format!("Rounds: {}, Total: ${}{}", 
            app.game_stats.len(),
            if total_profit >= 0 { "" } else { "-" }, 
            total_profit.abs())
    } else {
        "No rounds played".to_string()
    };
    
    // Game controls
    let controls = if app.game_active {
        "s: stop | q: quit"
    } else {
        "d: deal | n: set name | q: quit"
    };
    
    let status_widget = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("ACTIVE", Style::default().fg(Color::White))
        ]),
        Line::from(vec![
            Span::styled(format!("{} ({} bots)", 
                active_players, app.game.players.len() - 1), 
                Style::default().fg(Color::Blue))
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![
            Span::styled("LAST ROUND", Style::default().fg(Color::White))
        ]),
        Line::from(vec![
            Span::styled(result_display, Style::default().fg(Color::Green))
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![
            Span::styled("STATS", Style::default().fg(Color::White))
        ]),
        Line::from(vec![
            Span::styled(stats_display, Style::default().fg(Color::Yellow))
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![
            Span::styled("CONTROLS", Style::default().fg(Color::White))
        ]),
        Line::from(vec![
            Span::styled(controls, Style::default().fg(Color::Cyan))
        ])
    ])
    .block(Block::default().title("Info").borders(Borders::ALL));
    f.render_widget(status_widget, area);
}

fn render_community_cards<B: Backend>(f: &mut Frame<B>, app: &App, area: tui::layout::Rect) {
    // Community cards - ensure they don't overflow
    let community_text = if app.game.community_cards.is_empty() {
        "No community cards yet".to_string()
    } else {
        let cards_text = app.game.community_cards.iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        
        // Truncate if necessary to prevent overflow
        if cards_text.len() > area.width as usize - 4 {
            format!("{}..", &cards_text[0..(area.width as usize - 7)])
        } else {
            cards_text
        }
    };
    
    let community = Paragraph::new(community_text)
        .block(Block::default().title("Community Cards").borders(Borders::ALL));
    f.render_widget(community, area);
}

// Render the player's hand
fn render_player_hand<B: Backend>(f: &mut Frame<B>, app: &App, area: tui::layout::Rect) {
    // Player's hand - prevent overflow
    let hand_text = app.game.players.iter()
        .find(|p| !p.is_bot)
        .map(|p| {
            p.hand.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| "No cards".to_string());
    
    // Truncate if necessary to prevent overflow
    let hand_text = if hand_text.len() > area.width as usize - 4 {
        format!("{}..", &hand_text[0..(area.width as usize - 7)])
    } else {
        hand_text
    };
    
    let hand_block = Block::default()
        .title("Your Hand")
        .borders(Borders::ALL);
        
    let hand_widget = Paragraph::new(hand_text)
        .block(hand_block);
        
    f.render_widget(hand_widget, area);
}

// Render the message log with scrolling
fn render_messages<B: Backend>(f: &mut Frame<B>, app: &mut App, area: tui::layout::Rect) {
    // Messages - improve formatting and handle small screens
    // Calculate max message width with safety margin to prevent overflow
    let max_msg_width = if area.width > 10 { area.width as usize - 8 } else { 2 };
    
    // Keep more history and allow scrolling
    // Display all messages without limit for scrolling
    let messages: Vec<ListItem> = app.messages.iter()
        .map(|m| {
            // More aggressive truncation for messages
            let display_msg = if m.len() > max_msg_width {
                // Ensure we don't go out of bounds with very small windows
                let end_pos = if max_msg_width > 5 { max_msg_width - 3 } else { 2 };
                if end_pos < m.len() {
                    format!("{}..", &m[0..end_pos])
                } else {
                    m.clone()
                }
            } else {
                m.clone()
            };
            
            // Use appropriate styling for different message types
            if m.contains("wins") || m.contains("won") {
                ListItem::new(vec![Line::from(vec![
                    Span::styled(display_msg, Style::default().fg(Color::Green))
                ])])
            } else if m.contains("lost") || m.contains("error") || m.contains("fold") {
                ListItem::new(vec![Line::from(vec![
                    Span::styled(display_msg, Style::default().fg(Color::Red))
                ])])
            } else if m.contains("Your") || m.contains("You") {
                ListItem::new(vec![Line::from(vec![
                    Span::styled(display_msg, Style::default().fg(Color::Cyan))
                ])])
            } else {
                ListItem::new(vec![Line::from(vec![Span::raw(display_msg)])])
            }
        })
        .collect();
    
    // Create scrollable list using StatefulList
    let mut messages_state = ListState::default();
    
    // Auto-scroll to bottom if not manually scrolled up,
    // otherwise keep user's scroll position
    let messages_len = messages.len();
    
    if messages_len > 0 {
        // If user hasn't scrolled up manually or we're adding new messages
        if app.message_scroll_pos == 0 || app.message_scroll_pos >= messages_len.saturating_sub(2) {
            // Auto-scroll to bottom
            app.message_scroll_pos = messages_len.saturating_sub(1);
        }
        
        // This ensures the selected item is always visible
        messages_state.select(Some(app.message_scroll_pos.min(messages_len.saturating_sub(1))));
    } else {
        // Empty message list
        app.message_scroll_pos = 0;
    }
    
    // Create a scrollable style with visual indication
    let title_text = if messages_len > 0 {
        format!("Game Log (Scrollable ↑↓ - {}/{})", 
                app.message_scroll_pos.saturating_add(1), 
                messages_len)
    } else {
        "Game Log (Empty)".to_string()
    };
    
    let messages_widget = List::new(messages)
        .block(Block::default()
            .title(title_text)
            .borders(Borders::ALL))
        .highlight_style(Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD));
    
    // Render with state to enable scrolling
    f.render_stateful_widget(messages_widget, area, &mut messages_state);
}

// Render the input field
fn render_input<B: Backend>(f: &mut Frame<B>, app: &App, area: tui::layout::Rect) {
    // Input with enhanced info about available commands including scroll hints
    let input_title = if app.input_mode == crate::app::InputMode::PlayerName {
        "Input [Enter name, press 'n' to confirm]".to_string()
    } else if app.game_active && !app.bot_thinking && !app.game.players[app.game.current_player_idx].is_bot {
        // Show appropriate options based on the current betting situation and player's chips
        let highest_bet = app.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        let player = &app.game.players[app.game.current_player_idx];
        let player_current_bet = player.current_bet;
        let player_chips = player.chips;
        
        // Determine available actions
        let mut available_actions = Vec::new();
        
        // Check/Call option
        if highest_bet > player_current_bet {
            if player_chips > 0 {
                available_actions.push("[c]all");
            }
        } else {
            available_actions.push("[k]heck");
        }
        
        // Fold option - always available unless checking is free
        if highest_bet > player_current_bet || player_current_bet > 0 {
            available_actions.push("[f]old");
        }
        
        // Raise option - only if player has enough chips for min raise
        let min_raise_amount = highest_bet * 2;
        if player_chips > (highest_bet - player_current_bet) {
            // Only show raise if player has chips left after calling
            if player_chips > (highest_bet - player_current_bet) + app.game.min_bet {
                available_actions.push("[r]aise");
            }
        }
        
        if available_actions.is_empty() {
            "Input [WAITING...]".to_string()
        } else {
            format!("Input [{}]", available_actions.join(" "))
        }
    } else if app.bot_thinking {
        "Input [WAITING...]".to_string()
    } else {
        "Input [d:deal q:quit]".to_string()
    };
    
    // Truncate input if it gets too long
    let display_input = if app.input.len() > area.width as usize - 6 {
        format!("{}..", &app.input[0..(area.width as usize - 9)])
    } else {
        app.input.clone()
    };
    
    // Also truncate the title if needed
    let truncated_title = if input_title.len() > area.width as usize - 6 {
        format!("{}..", &input_title[0..(area.width as usize - 9)])
    } else {
        input_title.to_string()
    };
    
    let input = Paragraph::new(display_input)
        .style(Style::default())
        .block(Block::default().title(truncated_title).borders(Borders::ALL));
    f.render_widget(input, area);
}
