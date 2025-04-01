mod game;
mod app;
mod util;

use std::io;
use std::time::Duration;
use rand::Rng;
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Style, Modifier, Color},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};

use app::App;
use game::Round;
use util::get_player_position;

fn main() -> Result<(), io::Error> {
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut app = App::new(api_key, "Player 1".to_string());
    
    // Main game loop
    loop {
        if app.game_active && app.game.players[app.game.current_player_idx].is_bot {
            if app.bot_thinking {
                if std::time::Instant::now() >= app.bot_think_until {
                    app.bot_thinking = false;
                } else {
                    std::thread::sleep(Duration::from_millis(50));
                }
            } else {
                let bot_player = &app.game.players[app.game.current_player_idx].clone();
                app.game.last_action_count += 1;
                
                if app.game.round != Round::PreFlop && app.game.community_cards.is_empty() {
                    app.messages.push(format!("Dealing cards for {:?} round", app.game.round));
                    app.game.deal_community_cards();
                }
                
                // Get bot action
                match app.game.get_bot_action(bot_player) {
                    Ok(bot_action) => {
                        let _bot_intent = &bot_action;
                        let bot_position = get_player_position(&app.game, app.game.current_player_idx);
                        let actual_action = app.game.perform_action(bot_action);
                        
                        let highest_bet = app.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
                        let is_first_bet = highest_bet == 0 || highest_bet == app.game.min_bet;
                        
                        let actual_action_str = match &actual_action.0 {
                            game::GameAction::Fold => "folds".to_string(),
                            game::GameAction::Call => "calls".to_string(),
                            game::GameAction::Check => "checks".to_string(),
                            game::GameAction::Raise(amount) => {
                                if is_first_bet && app.game.round != Round::PreFlop {
                                    if let Some(total) = actual_action.1 {
                                        format!("bets {}", total)
                                    } else {
                                        format!("bets {}", amount)
                                    }
                                } else {
                                    if let Some(total) = actual_action.1 {
                                        format!("raises to {}", total)
                                    } else {
                                        format!("raises by {}", amount)
                                    }
                                }
                            },
                        };
                        
                        app.messages.push(format!("{} in {} position {}.", bot_player.name, bot_position, actual_action_str));
                        let _human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                        let player_idx = app.game.current_player_idx;
                        let contribution = match &actual_action.0 {
                            game::GameAction::Call | game::GameAction::Raise(_) => {
                                app.game.pot - (app.game.pot - match &actual_action.0 {
                                    game::GameAction::Call => {
                                        if let Some(bet) = actual_action.1 {
                                            let previous_bet = app.game.players[player_idx].current_bet - 
                                                (highest_bet.saturating_sub(app.game.players[player_idx].current_bet)
                                                .min(app.game.players[player_idx].chips));
                                            bet - previous_bet
                                        } else { 0 }
                                    },
                                    game::GameAction::Raise(amount) => *amount,
                                    _ => 0
                                })
                            },
                            _ => 0
                        };
                        
                        let old_pot = if contribution > 0 {
                            app.game.pot - contribution
                        } else {
                            app.game.pot
                        };
                        
                        if old_pot < app.game.pot {
                            app.messages.push(format!("Pot increased from ${} to ${}.", old_pot, app.game.pot));
                        }
                        
                        let current_round = app.game.round;
                        let game_continues = app.game.next_player();
                        
                        if game_continues && app.game.players[app.game.current_player_idx].is_bot {
                            app.bot_thinking = true;
                            
                            let was_hand_just_dealt = app.messages.iter()
                                .rev()
                                .take(5)
                                .any(|msg| msg.contains("New hand dealt"));
                                
                            if was_hand_just_dealt {
                                app.bot_think_until = std::time::Instant::now() + 
                                    Duration::from_millis(rand::thread_rng().gen_range(3500..5000));
                            } else {
                                app.bot_think_until = std::time::Instant::now() + 
                                    Duration::from_millis(rand::thread_rng().gen_range(1000..2500));
                            }
                        }
                        
                        let new_round = app.game.round;
                        if new_round != current_round {
                            match new_round {
                                Round::Flop => {
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                    app.messages.push("--- Moving to FLOP round (first 3 community cards) ---".to_string());
                                },
                                Round::Turn => {
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                    app.messages.push("--- Moving to TURN round (4th community card) ---".to_string());
                                },
                                Round::River => {
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                    app.messages.push("--- Moving to RIVER round (final community card) ---".to_string());
                                },
                                Round::Showdown => {
                                    let bet_made_on_river = app.messages.iter().any(|msg| 
                                        (msg.contains("bet") || msg.contains("raise")) && 
                                        app.messages.iter().any(|m| m.contains("RIVER"))
                                    );
                                    
                                    if !bet_made_on_river {
                                        let players_to_show = app.game.players.iter()
                                            .enumerate()
                                            .filter(|(_, p)| !p.folded)
                                            .collect::<Vec<_>>();
                                        
                                        if app.messages.last().map_or(true, |msg| !msg.contains("checks")) {
                                            for (idx, player) in players_to_show {
                                                if !player.is_bot {
                                                    app.messages.push(format!("You check."));
                                                } else {
                                                    let position = get_player_position(&app.game, idx);
                                                    app.messages.push(format!("{} in {} position checks.", player.name, position));
                                                }
                                            }
                                        }
                                    }
                                    
                                    std::thread::sleep(std::time::Duration::from_millis(50));
                                    app.messages.push("--- Moving to SHOWDOWN (comparing hands) ---".to_string());
                                    app.messages.push("--- SHOWDOWN: Players reveal their hands ---".to_string());
                                    for (idx, player) in app.game.players.iter().enumerate() {
                                        if !player.folded && player.hand.len() >= 2 {
                                            let hand_str = player.hand.iter()
                                                .map(|c| c.to_string())
                                                .collect::<Vec<_>>()
                                                .join(" ");
                                                
                                            let position = get_player_position(&app.game, idx);
                                            
                                            if player.is_bot {
                                                app.messages.push(format!("{} ({}) shows: {}", player.name, position, hand_str));
                                            } else {
                                                app.messages.push(format!("You ({}) show: {}", position, hand_str));
                                            }
                                        }
                                    }
                                    
                                    // Force UI update with extra delay
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                    
                                    // In Showdown, we should immediately determine the winner
                                    // This eliminates the need for the player to act again
                                    let (winner_idx, winnings, hand_type) = app.game.determine_winner();
                                    let winner_name = app.game.players[winner_idx].name.clone();
                                    
                                    // Calculate profit/loss for human player
                                    let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                                    let human_player = &app.game.players[human_idx];
                                    let profit = human_player.chips as i32 - app.player_starting_chips as i32;
                                    
                                    // Set round results and track game stats
                                    app.round_results = Some((winner_name.clone(), profit));
                                    app.game_stats.push(profit);
                                    
                                    // Calculate total profit across all rounds
                                    let total_profit = app.game_stats.iter().sum::<i32>();
                                    
                                    // Add hand explanation based on hand type
                                    let hand_explanation = match hand_type.split_whitespace().next().unwrap_or("") {
                                        "Pair" => "A pair is two cards of the same rank.",
                                        "Two" => "Two pair means two different pairs of cards.",
                                        "Three" => "Three of a Kind is three cards of the same rank.",
                                        "Straight" => "A straight is five cards in sequential rank.",
                                        "Flush" => "A flush is five cards of the same suit.",
                                        "Full" => "A full house is three of a kind plus a pair.",
                                        "Four" => "Four of a Kind is four cards of the same rank.",
                                        "Straight-Flush" => "A straight flush is a straight and flush combined.",
                                        "Royal" => "A royal flush is A-K-Q-J-10 of the same suit - the best hand!",
                                        _ => "",
                                    };
                                    
                                    // Show community cards used in the win
                                    let community_display = if !app.game.community_cards.is_empty() {
                                        let cards = app.game.community_cards.iter()
                                            .map(|c| c.to_string())
                                            .collect::<Vec<_>>()
                                            .join(" ");
                                        format!(" (with community cards: {})", cards)
                                    } else {
                                        "".to_string()
                                    };
                                    
                                    // Display results in message log with more detail
                                    let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
                                    let formatted_message = format!("Round over! {} wins ${} chips with {}{}!", 
                                                            app.game.players[winner_idx].name, display_winnings, 
                                                            hand_type, community_display);
                                    app.messages.push(formatted_message);
                                    
                                    // Add explanation if available
                                    if !hand_explanation.is_empty() {
                                        app.messages.push(format!("Hand info: {}", hand_explanation));
                                    }
                                    
                                    if winner_idx == human_idx {
                                        app.messages.push(format!("You won this hand! Your profit: ${}. Total: ${}", profit.abs(), total_profit));
                                    } else {
                                        app.messages.push(format!("You lost this hand. Your loss: ${}. Total: ${}", profit.abs(), total_profit));
                                    }
                                    
                                    // Print game stats
                                    app.print_game_stats();
                                    
                                    // End the game
                                    app.game_active = false;
                                    app.messages.push("Press 'd' to deal a new hand.".to_string());
                                    app.messages.push("".to_string()); // Add empty line between rounds
                                    
                                    // Ensure the message scroll position is updated to show the latest messages
                                    app.message_scroll_pos = app.messages.len().saturating_sub(1);
                                    
                                    // Force UI update with one more delay
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                    
                                    // Continue processing but with longer delay
                                    // Setting game_active = false above is enough
                                    // This prevents abrupt exits
                                    // Add additional delay to ensure all messages are shown
                                    std::thread::sleep(std::time::Duration::from_millis(200));
                                },
                                _ => {}
                            }
                            
                            // Log the new community cards if appropriate
                            if !app.game.community_cards.is_empty() {
                                let cards_text = app.game.community_cards.iter()
                                    .map(|c| c.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                app.messages.push(format!("Community cards: {}", cards_text));
                                
                                // Force UI update by adding a small delay
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            
                            // Make sure the current player is correctly set for the new round
                            if app.game.round != Round::Showdown && !app.game.players[app.game.current_player_idx].is_bot {
                                // Human's turn - notify explicitly
                                // Check if there's a bet to call
                                let highest_bet = app.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
                                let player_current_bet = app.game.players[app.game.current_player_idx].current_bet;
                                
                                if highest_bet > player_current_bet {
                                    app.messages.push(format!("Your turn now. Choose action: [c]all, [f]old, or [r]aise."));
                                } else {
                                    app.messages.push(format!("Your turn. No bet to call. Choose [k]heck or [r]aise."));
                                }
                            }
                        }
                        
                        // Check if round ended
                        if !game_continues {
                            // Get winner info
                            let (winner_idx, winnings, hand_type) = app.game.determine_winner();
                            let winner_name = app.game.players[winner_idx].name.clone();
                            
                            // Calculate profit/loss for human player
                            let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                            let human_player = &app.game.players[human_idx];
                            let profit = human_player.chips as i32 - app.player_starting_chips as i32;
                            
                            // Set round results and track game stats
                            app.round_results = Some((winner_name.clone(), profit));
                            app.game_stats.push(profit);
                            
                            // Calculate total profit across all rounds
                            let total_profit = app.game_stats.iter().sum::<i32>();
                            
                            // Display results in message log with minimum winnings - use shorter messages
                            let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
                            app.messages.push(format!("{} wins ${} with {}!", 
                                                   app.game.players[winner_idx].name, display_winnings, hand_type));
                            
                            if winner_idx == human_idx {
                                app.messages.push(format!("You won! Profit: ${}. Total: ${}", profit.abs(), total_profit));
                            } else {
                                app.messages.push(format!("You lost. Loss: ${}. Total: ${}", profit.abs(), total_profit));
                            }
                            
                            // Mark game as inactive until player deals again
                            app.game_active = false;
                            app.messages.push("Press 'd' to deal a new hand.".to_string());
                        } else if app.game.players[app.game.current_player_idx].is_bot {
                            // If next player is a bot, set realistic thinking time
                            app.bot_thinking = true;
                            app.bot_think_until = std::time::Instant::now() + 
                                Duration::from_millis(rand::thread_rng().gen_range(1500..3000));
                        }
                        
                        // Safety check to prevent infinite loop
                        if app.game.last_action_count > 25 { // Increased from 15 to 25 to allow more actions
                            app.messages.push("Round ending (action limit reached).".to_string());
                            let (winner_idx, winnings, hand_type) = app.game.determine_winner();
                            // Use minimum winnings display here too with shorter format
                            let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
                            // Add to game stats
                            let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                            let human_player = &app.game.players[human_idx];
                            let profit = human_player.chips as i32 - app.player_starting_chips as i32;
                            app.game_stats.push(profit);
                            let total_profit = app.game_stats.iter().sum::<i32>();
                            
                            // Add hand explanation based on hand type
                            let hand_explanation = match hand_type.split_whitespace().next().unwrap_or("") {
                                "Pair" => "A pair is two cards of the same rank.",
                                "Two" => "Two pair means two different pairs of cards.",
                                "Three" => "Three of a Kind is three cards of the same rank.",
                                "Straight" => "A straight is five cards in sequential rank.",
                                "Flush" => "A flush is five cards of the same suit.",
                                "Full" => "A full house is three of a kind plus a pair.",
                                "Four" => "Four of a Kind is four cards of the same rank.",
                                "Straight-Flush" => "A straight flush is a straight and flush combined.",
                                "Royal" => "A royal flush is A-K-Q-J-10 of the same suit - the best hand!",
                                _ => "",
                            };
                            
                            // Show community cards used in the win
                            let community_display = if !app.game.community_cards.is_empty() {
                                let cards = app.game.community_cards.iter()
                                    .map(|c| c.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                format!(" (with community cards: {})", cards)
                            } else {
                                "".to_string()
                            };
                            
                            let formatted_result = format!("{} wins ${} with {}{}! Your total profit: ${}", 
                                                    app.game.players[winner_idx].name, display_winnings, 
                                                    hand_type, community_display, total_profit);
                            app.messages.push(formatted_result);
                            
                            // Add explanation if available
                            if !hand_explanation.is_empty() {
                                app.messages.push(format!("Hand info: {}", hand_explanation));
                            }
                            
                            app.game.last_action_count = 0;
                            
                            // Print game stats
                            app.print_game_stats();
                            
                            app.game_active = false;
                            app.messages.push("Press 'd' to deal a new hand.".to_string());
                            app.messages.push("".to_string()); // Add empty line between rounds
                        }
                    },
                    Err(e) => {
                        app.messages.push(format!("Bot error: {}", e));
                        // End the game on error to prevent loops
                        app.game_active = false;
                    }
                }
            }
        }
        
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(7),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ].as_ref())
                .split(f.size());
            
            let current_player = &app.game.players[app.game.current_player_idx];
            let current_player_name = &current_player.name;
            let turn_info = if !app.game_active {
                "Press 'd' to deal, 'q' to quit"
            } else if !current_player.is_bot {
                "Your turn."
            } else if app.bot_thinking {
                &format!("{} thinking...", current_player_name)
            } else {
                &format!("Waiting for {}", current_player_name)
            };
            
            // Add turn information to the message log when it changes
            if app.game_active && !current_player.is_bot && 
               app.messages.last().map_or(true, |msg| !msg.contains("Your turn")) {
                // Only add this message if we haven't added it recently (avoid duplicates)
                if app.messages.len() < 2 || !app.messages[app.messages.len() - 2].contains("Your turn") {
                    // Check if there's a bet to call
                    let highest_bet = app.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
                    let player_current_bet = app.game.players[app.game.current_player_idx].current_bet;
                    
                    let turn_message = if highest_bet > player_current_bet {
                        "Your turn now. Choose action: [c]all, [f]old, or [r]aise."
                    } else {
                        "Your turn. No bet to call. Choose [k]heck or [r]aise."
                    };
                    
                    app.messages.push(turn_message.to_string());
                    // Keep the messages list scrolled to the bottom to show this message
                    app.message_scroll_pos = app.messages.len().saturating_sub(1);
                }
            }
            
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
                    if app.bot_thinking && player.is_bot {
                        "꘎"  // Thinking
                    } else {
                        "➤"   // Current turn
                    }
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
                
                player_status.push_str(&format!("{}:{} ", display_name, status));
            }
            
            // Indicate if more players aren't shown
            if app.game.players.len() > max_players_to_show {
                player_status.push_str(&format!("(+{})", app.game.players.len() - max_players_to_show));
            }
            
            // Game info
            let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
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
            
            // Game status/controls display
            let game_status = if app.game_active {
                "Game in progress [s: stop game]"
            } else {
                "Game not active [d: deal new hand, q: quit]"
            };

            let pot_style = if app.game.pot > 100 {
                Style::default().fg(Color::Yellow)
            } else if app.game.pot > 50 {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };
            
            let total_width = f.size().width as usize - 4;
            let truncate_large = total_width < 70;
            
            let game_info = Paragraph::new(vec![
                Line::from(vec![
                    Span::raw("Pot: "),
                    Span::styled(format!("${} ", app.game.pot), pot_style),
                    Span::raw("| "),
                    Span::raw(if truncate_large { "Chips: " } else { "Your Chips: " }),
                    Span::styled(format!("${} ", 
                        app.game.players.iter()
                            .find(|p| !p.is_bot)
                            .map(|p| p.chips)
                            .unwrap_or(0)
                    ), Style::default().fg(Color::Cyan)),
                    Span::raw("| "),
                    Span::raw(if truncate_large { "Bet: " } else { "Current Bet: " }),
                    Span::styled(format!("${}", 
                        app.game.players.iter()
                            .map(|p| p.current_bet)
                            .max()
                            .unwrap_or(0)
                    ), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(vec![
                    Span::raw("Round: "),
                    Span::styled(format!("{:?}", app.game.round), Style::default().fg(Color::Green)),
                    Span::raw(" | Active Players: "),
                    Span::styled(format!("{} ({} bots)", active_players, app.game.players.len() - 1), 
                                Style::default().fg(Color::Blue)),
                    Span::raw(" | "),
                    Span::raw(if truncate_large { "Pos: " } else { "Position: " }),
                    Span::styled(
                        // Truncate position name if too long
                        if human_position.len() > 15 && truncate_large {
                            format!("{}...", &human_position[0..12])
                        } else {
                            human_position
                        }, 
                        Style::default().fg(Color::Cyan)
                    ),
                ]),
                // Row 3: Table positions (with potential truncation)
                Line::from(vec![
                    Span::raw("D: "),
                    Span::styled(
                        // Truncate dealer name if too long
                        if app.game.players[app.game.dealer_idx].name.len() > 10 && truncate_large {
                            format!("{}...", &app.game.players[app.game.dealer_idx].name[0..7])
                        } else {
                            app.game.players[app.game.dealer_idx].name.clone()
                        },
                        Style::default().fg(Color::Yellow)
                    ),
                    Span::raw(" | SB: "),
                    Span::styled(
                        // Truncate SB name if too long
                        if app.game.players[app.game.small_blind_idx].name.len() > 10 && truncate_large {
                            format!("{}...", &app.game.players[app.game.small_blind_idx].name[0..7])
                        } else {
                            app.game.players[app.game.small_blind_idx].name.clone()
                        },
                        Style::default().fg(Color::Yellow)
                    ),
                    Span::raw(" | BB: "),
                    Span::styled(
                        // Truncate BB name if too long
                        if app.game.players[app.game.big_blind_idx].name.len() > 10 && truncate_large {
                            format!("{}...", &app.game.players[app.game.big_blind_idx].name[0..7])
                        } else {
                            app.game.players[app.game.big_blind_idx].name.clone()
                        },
                        Style::default().fg(Color::Yellow)
                    ),
                ]),
                // Row 4: Player status (with truncation to prevent overflow)
                Line::from(vec![
                    Span::raw("Players: "),
                    Span::styled(
                        // Ensure player status fits within available width
                        if player_status.len() + 10 > total_width {
                            // Safe truncation with bounds checking
                            let safe_len = total_width.saturating_sub(13);
                            if safe_len > 0 && safe_len < player_status.len() {
                                format!("{}...", &player_status[0..safe_len])
                            } else {
                                player_status.chars().take(total_width.saturating_sub(13)).collect::<String>()
                            }
                        } else {
                            player_status
                        }, 
                        Style::default().fg(Color::White))
                ]),
                // Row 5: Game stats or turn info (with truncation for long texts)
                Line::from(vec![
                    Span::styled("► ", Style::default().fg(Color::Green)),
                    Span::styled(
                        if !app.game_active && !app.game_stats.is_empty() {
                            let total_profit = app.game_stats.iter().sum::<i32>();
                            let display = format!("Total profit: ${}. Rounds played: {}", 
                                                total_profit, app.game_stats.len());
                            if display.len() + 2 > total_width {
                                format!("{}...", &display[0..total_width.saturating_sub(5)])
                            } else {
                                display
                            }
                        } else if turn_info.len() + 2 > total_width {
                            format!("{}...", &turn_info[0..total_width.saturating_sub(5)])
                        } else {
                            turn_info.to_string()
                        }, 
                        Style::default().fg(Color::Cyan))
                ]),
                // Row 6: Last result and game status (with truncation)
                Line::from(vec![
                    Span::styled(
                        if result_display.len() > 35 {
                            format!("{}...", &result_display[0..32]) 
                        } else {
                            result_display.to_string()
                        },
                        Style::default().fg(Color::Green)
                    ),
                    Span::raw("   "),
                    Span::styled(
                        if game_status.len() > 35 {
                            format!("{}...", &game_status[0..32])
                        } else {
                            game_status.to_string()
                        },
                        Style::default().fg(Color::Yellow)
                    )
                ])
            ])
            .block(Block::default().title("").borders(Borders::ALL));
            f.render_widget(game_info, chunks[0]);
            
            // Community cards - ensure they don't overflow
            let community_text = if app.game.community_cards.is_empty() {
                "No community cards yet".to_string()
            } else {
                let cards_text = app.game.community_cards.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                
                // Truncate if necessary to prevent overflow
                if cards_text.len() > f.size().width as usize - 4 {
                    format!("{}...", &cards_text[0..(f.size().width as usize - 7)])
                } else {
                    cards_text
                }
            };
            
            let community = Paragraph::new(community_text)
                .block(Block::default().title("Community Cards").borders(Borders::ALL));
            f.render_widget(community, chunks[1]);
            
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
            let hand_text = if hand_text.len() > f.size().width as usize - 4 {
                format!("{}...", &hand_text[0..(f.size().width as usize - 7)])
            } else {
                hand_text
            };
            
            let hand_block = Block::default()
                .title("Your Hand")
                .borders(Borders::ALL);
                
            let hand_widget = Paragraph::new(hand_text)
                .block(hand_block);
                
            f.render_widget(hand_widget, chunks[2]);
            
            let max_msg_width = if f.size().width > 10 { f.size().width as usize - 8 } else { 2 };
            
            let messages: Vec<ListItem> = app.messages.iter()
                .map(|m| {
                    let display_msg = if m.len() > max_msg_width {
                        let end_pos = if max_msg_width > 5 { max_msg_width - 3 } else { 2 };
                        format!("{}...", &m[0..end_pos])
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
                    } else if m.contains("thinking") {
                        ListItem::new(vec![Line::from(vec![
                            Span::styled(display_msg, Style::default().fg(Color::Yellow))
                        ])])
                    } else {
                        ListItem::new(vec![Line::from(vec![Span::raw(display_msg)])])
                    }
                })
                .collect();
            
            let messages_state = &mut ListState::default();
            
            if !messages.is_empty() {
                if app.message_scroll_pos == 0 || messages.len() < 3 || app.message_scroll_pos >= messages.len().saturating_sub(2) {
                    app.message_scroll_pos = messages.len().saturating_sub(1);
                }
                
                messages_state.select(Some(app.message_scroll_pos.min(messages.len().saturating_sub(1))));
            }
            
            // Create a scrollable style with visual indication
            let messages_widget = List::new(messages)
                .block(Block::default()
                    .title(format!("Game Log (Scrollable - {}/{})", 
                                   app.message_scroll_pos + 1, 
                                   app.messages.len()))
                    .borders(Borders::ALL))
                .highlight_style(Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD));
            
            // Render with state to enable scrolling
            f.render_stateful_widget(messages_widget, chunks[3], messages_state);
            
            // Input with enhanced info about available commands including scroll hints
            let input_title = if app.input_mode == app::InputMode::PlayerName {
                "Input [Enter name, press 'n' to confirm]"
            } else if app.game_active && !app.bot_thinking && !app.game.players[app.game.current_player_idx].is_bot {
                "Input"
            } else if app.bot_thinking {
                "Input [THINKING...]"
            } else if !app.game_active {
                "Input [d:deal q:quit]"
            } else {
                "Input [WAITING FOR YOUR TURN...]"
            };
            
            let display_input = if app.input.len() > f.size().width as usize - 6 {
                format!("{}...", &app.input[0..(f.size().width as usize - 9)])
            } else {
                app.input.clone()
            };
            
            let truncated_title = if input_title.len() > f.size().width as usize - 6 {
                format!("{}...", &input_title[0..(f.size().width as usize - 9)])
            } else {
                input_title.to_string()
            };
            
            let input = Paragraph::new(display_input)
                .style(Style::default())
                .block(Block::default().title(truncated_title).borders(Borders::ALL));
            f.render_widget(input, chunks[4]);
        })?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key.code);
                if app.should_quit {
                    break;
                }
            }
        }
    }
    
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    
    Ok(())
}