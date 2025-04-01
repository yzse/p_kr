        let starting_chips = 1000;
        
        // Set up a game with 1 human player and 8 bots (total 9 players)
        let game = Game::new(1, 8, BotDifficulty::Medium, starting_chips, api_key, player_name);
        
        // Create initial instructions
        let initial_messages = vec![
            "Welcome to P_kr Poker!".to_string(),
            "Enter your name and press 'n' to set it, or press Enter for default 'Player 1'.".to_string(),
            "Press 'd' to deal a new hand, 'q' to quit.".to_string(),
            "During play, use 'k' to check, 'c' to call, 'f' to fold, or type a number and press 'r' to raise.".to_string(),
        ];
        
        App {
            game,
            input: String::new(),
            messages: initial_messages,
            should_quit: false,
            player_starting_chips: starting_chips,
            round_results: None,
            game_stats: Vec::new(),
            bot_thinking: false,
            bot_think_until: std::time::Instant::now(),
            game_active: false,
            message_scroll_pos: 4, // Start at bottom of instructions
            input_mode: InputMode::Normal
        }
    }
    
    fn on_key(&mut self, key: KeyCode) {
        // Don't process input when bot is thinking or it's not the player's turn
        let is_player_turn = !self.game.players[self.game.current_player_idx].is_bot;
        let can_take_action = is_player_turn && !self.bot_thinking;
        
        // Handle input based on current input mode
        match self.input_mode {
            InputMode::PlayerName => {
                // Special handling for player name input
                match key {
                    KeyCode::Char('n') => {
                        // Set the player name if input is not empty
                        if !self.input.is_empty() {
                            let new_name = self.input.clone();
                            let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                            self.game.players[human_idx].name = new_name.clone();
                            self.messages.push(format!("Your name has been set to '{}'.", new_name));
                            self.input.clear();
                            self.input_mode = InputMode::Normal;
                        } else {
                            self.messages.push("Name cannot be empty. Please enter a name.".to_string());
                        }
                    },
                    KeyCode::Char(c) => {
                        // Allow any character for name
                        self.input.push(c);
                    },
                    KeyCode::Backspace => {
                        self.input.pop();
                    },
                    _ => {}
                }
            },
            InputMode::Normal => {
                // Regular game input handling
                match key {
                    KeyCode::Char('q') => {
                        self.should_quit = true;
                    },
                    KeyCode::Char('d') => {
                        // Allow starting new hand even if there's a game in progress
                        self.game.deal_cards();
                        self.messages.push("New hand dealt.".to_string());
                        
                        // Verify deck is properly set up - must have more than 2*players cards 
                        // after initial deal (approximately 52 - 2*player_count)
                        if self.game.deck.len() < 35 {
                            // Silently replace the deck without printing warnings
                            self.game.deck = Game::create_deck();
                            self.game.shuffle_deck();
                        }
                        
                        // Reset tracking for new hand
                        let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                        self.player_starting_chips = self.game.players[human_idx].chips;
                        self.round_results = None;
                        self.game_active = true;
                        self.bot_thinking = false;
                        self.game.last_action_count = 0;
                        
                        // Show total stats
                        if !self.game_stats.is_empty() {
                            let total_profit = self.game_stats.iter().sum::<i32>();
                            let profit_list = self.game_stats.iter()
                                .enumerate()
                                .map(|(i, profit)| format!("R{}: ${}{}", i+1, if *profit >= 0 {""} else {"-"}, profit.abs()))
                                .collect::<Vec<_>>()
                                .join(", ");
                            self.messages.push(format!("Game stats: {} rounds played. Profits: {}. Total: ${}", 
                                                      self.game_stats.len(), profit_list, total_profit));
                        }
                    },
                    KeyCode::Char('n') => {
                        // Switch to player name input mode
                        self.input.clear();
                        self.input_mode = InputMode::PlayerName;
                        self.messages.push("Enter your name and press 'n' to confirm:".to_string());
                    },
                    KeyCode::Char('s') => {
                        // Stop current game
                        if self.game_active {
                            self.game_active = false;
                            self.bot_thinking = false;
                            self.messages.push("Game stopped. Press 'd' to deal a new hand.".to_string());
                        }
                    },
                    KeyCode::Char('c') => {
                        // Allow player action regardless of round
                        if can_take_action && self.game_active {
                            // Double-check it's actually the player's turn
                            if !self.game.players[self.game.current_player_idx].is_bot {
                                self.handle_player_action(GameAction::Call);
                            } else {
                                self.messages.push("It's not your turn yet. Please wait.".to_string());
                            }
                        }
                    },
                    KeyCode::Char('k') => {
                        // Allow player action regardless of round
                        if can_take_action && self.game_active {
                            // Double-check it's actually the player's turn
                            if !self.game.players[self.game.current_player_idx].is_bot {
                                self.handle_player_action(GameAction::Check);
                            } else {
                                self.messages.push("It's not your turn yet. Please wait.".to_string());
                            }
                        }
                    },
                    KeyCode::Char('f') => {
                        // Allow player action regardless of round
                        if can_take_action && self.game_active {
                            // Double-check it's actually the player's turn
                            if !self.game.players[self.game.current_player_idx].is_bot {
                                self.handle_player_action(GameAction::Fold);
                            } else {
                                self.messages.push("It's not your turn yet. Please wait.".to_string());
                            }
                        }
                    },
                    KeyCode::Char('r') => {
                        // Allow player action regardless of round
                        if can_take_action && self.game_active {
                            // Double-check it's actually the player's turn
                            if !self.game.players[self.game.current_player_idx].is_bot {
                                // Use the current input as raise amount
                                if self.input.is_empty() {
                                    self.messages.push("Please enter a raise amount first, then press 'r'.".to_string());
                                } else if let Ok(amount) = self.input.parse::<u32>() {
                                    self.handle_player_action(GameAction::Raise(amount));
                                    self.input.clear();
                                } else {
                                    self.messages.push("Invalid raise amount. Please enter a number.".to_string());
                                }
                            } else {
                                self.messages.push("It's not your turn yet. Please wait.".to_string());
                            }
                        }
                    },
                    KeyCode::Char(c) => {
                        if c.is_digit(10) && is_player_turn {
                            self.input.push(c);
                        }
                    },
                    KeyCode::Backspace => {
                        if is_player_turn {
                            self.input.pop();
                        }
                            },
                    // Add scrolling support for message history
                    KeyCode::Up => {
                        if self.message_scroll_pos > 0 {
                            self.message_scroll_pos -= 1;
                        }
                    },
                    KeyCode::Down => {
                        if self.message_scroll_pos < self.messages.len().saturating_sub(1) {
                            self.message_scroll_pos += 1;
                        }
                    },
                    KeyCode::PageUp => {
                        // Scroll up 10 lines at a time
                        self.message_scroll_pos = self.message_scroll_pos.saturating_sub(10);
                    },
                    KeyCode::PageDown => {
                        // Scroll down 10 lines at a time
                        self.message_scroll_pos = (self.message_scroll_pos + 10).min(self.messages.len().saturating_sub(1));
                    },
                    KeyCode::Home => {
                        // Scroll to the top
                        self.message_scroll_pos = 0;
                    },
                    KeyCode::End => {
                        // Scroll to the bottom
                        self.message_scroll_pos = self.messages.len().saturating_sub(1);
                    },
                    _ => {}
                }
            }
        }
    }
    
    fn handle_player_action(&mut self, action: GameAction) {
        // Special handling for Showdown round - force winner determination
        if self.game.round == Round::Showdown {
            match action {
                GameAction::Fold => {
                    self.messages.push("Showdown in progress. Determining winner...".to_string());
                },
                GameAction::Call => {
                    self.messages.push("Showdown in progress. Determining winner...".to_string());
                },
                GameAction::Check => {
                    self.messages.push("Showdown in progress. Determining winner...".to_string());
                },
                GameAction::Raise(_) => {
                    self.messages.push("Showdown in progress. Determining winner...".to_string());
                }
            }
            
            // Force winner determination and round completion
            let (winner_idx, winnings, hand_type) = self.game.determine_winner();
            let winner_name = self.game.players[winner_idx].name.clone();
            
            // Calculate profit/loss for human player
            let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
            let human_player = &self.game.players[human_idx];
            let profit = human_player.chips as i32 - self.player_starting_chips as i32;
            
            // Set round results and track game stats
            self.round_results = Some((winner_name.clone(), profit));
            self.game_stats.push(profit);
            
            // Calculate total profit across all rounds
            let total_profit = self.game_stats.iter().sum::<i32>();
            
            // Display results in message log
            let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
            self.messages.push(format!("Round over! {} wins ${} chips with {}!", 
                                      self.game.players[winner_idx].name, display_winnings, hand_type));
            
            if winner_idx == human_idx {
                self.messages.push(format!("You won this hand! Your profit: ${}. Total: ${}", profit.abs(), total_profit));
            } else {
                self.messages.push(format!("You lost this hand. Your loss: ${}. Total: ${}", profit.abs(), total_profit));
            }
            
            // End the game
            self.game_active = false;
            self.messages.push("Press 'd' to deal a new hand.".to_string());
            return;
        }
        
        // Check if it's actually the player's turn
        let current_player_idx = self.game.current_player_idx;
        let is_current_player = !self.game.players[current_player_idx].is_bot;
        
        if !is_current_player {
            self.messages.push("It's not your turn yet. Please wait.".to_string());
            return;
        }
        
        // Check for missing community cards in non-preflop rounds
        if self.game.round != Round::PreFlop && self.game.community_cards.is_empty() {
            self.messages.push("Dealing community cards...".to_string());
            
            // Force round advancement if stuck in PreFlop but UI shows different round
            if self.game.round != Round::PreFlop && self.game.community_cards.is_empty() {
                // Force appropriate community cards based on round
                match self.game.round {
                    Round::Flop => {
                        self.game.community_cards.clear();
                        for _ in 0..3 {
                            if let Some(card) = self.game.deck.pop() {
                                self.game.community_cards.push(card);
                            }
                        }
                    },
                    Round::Turn => {
                        self.game.community_cards.clear();
                        for _ in 0..4 { // Flop + Turn = 4 cards
                            if let Some(card) = self.game.deck.pop() {
                                self.game.community_cards.push(card);
                            }
                        }
                    },
                    Round::River => {
                        self.game.community_cards.clear();
                        for _ in 0..5 { // Full board = 5 cards
                            if let Some(card) = self.game.deck.pop() {
                                self.game.community_cards.push(card);
                            }
                        }
                    },
                    _ => {
                        // Let normal deal_community_cards handle it
                        self.game.deal_community_cards();
                    }
                }
            } else {
                // Normal dealing
                self.game.deal_community_cards();
            }
            
            // Log what was dealt
            let cards_text = self.game.community_cards.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            self.messages.push(format!("Community cards: {}", cards_text));
        }
        
        let player_position = get_player_position(&self.game, self.game.current_player_idx);
        let action_str = match &action {
            GameAction::Fold => "fold".to_string(),
            GameAction::Call => "call".to_string(),
            GameAction::Check => "check".to_string(),
            GameAction::Raise(amount) => format!("raise {}", amount),
        };
        
        self.messages.push(format!("You (in {} position) {}.", player_position, action_str));
        self.game.perform_action(action);
        
        // Move to next player
        let game_continues = self.game.next_player();
        
        // Reset action counter when human makes a move
        self.game.last_action_count = 0;
        
        // Check if game ended after player's action
        if !game_continues {
            // Get winner info
            let (winner_idx, winnings, hand_type) = self.game.determine_winner();
            let winner_name = self.game.players[winner_idx].name.clone();
            
            // Calculate profit/loss for human player
            let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
            let human_player = &self.game.players[human_idx];
            let profit = human_player.chips as i32 - self.player_starting_chips as i32;
            
            // Set round results
            self.round_results = Some((winner_name, profit));
            
            // Display results in message log
            self.messages.push(format!("Round over! {} wins ${} chips with {}!", 
                                      self.game.players[winner_idx].name, winnings, hand_type));
            
            if winner_idx == human_idx {
                self.messages.push(format!("You won this hand! Your profit: ${}.", profit.abs()));
            } else {
                self.messages.push(format!("You lost this hand. Your loss: ${}.", profit.abs()));
            }
            return;
        }
        
        // If next player is a bot, set up initial thinking time
        if self.game.players[self.game.current_player_idx].is_bot {
            self.bot_thinking = true;
            self.bot_think_until = std::time::Instant::now() + 
                std::time::Duration::from_millis(rand::thread_rng().gen_range(500..1500));
        }
    }
}

fn main() -> Result<(), io::Error> {
    // Get API key from environment variable if available
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    
    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Use default player name initially
    let mut app = App::new(api_key, "Player 1".to_string());
    
    // Initialize app but don't automatically deal 
    app.messages.push("Press 'd' to deal a new hand.".to_string());
    
    // Main game loop
    loop {
        // Process bot actions if the game is active, it's a bot's turn and not in "thinking" mode
        if app.game_active && app.game.players[app.game.current_player_idx].is_bot {
            if app.bot_thinking {
                // Check if bot thinking time is over
                if std::time::Instant::now() >= app.bot_think_until {
                    app.bot_thinking = false;
                } else {
                    // Bot is still "thinking", don't do anything yet
                    // Just add a short delay for the UI to remain responsive
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            } else {
                let bot_player = &app.game.players[app.game.current_player_idx].clone();
                
                // Increment action counter to prevent infinite loops
                app.game.last_action_count += 1;
                
                // Debug check for community cards in appropriate rounds
                if app.game.round != Round::PreFlop && app.game.community_cards.is_empty() {
                    // Add to messages instead of printing
                    app.messages.push(format!("Dealing cards for {:?} round", app.game.round));
                    app.game.deal_community_cards();
                }
                
                // Get bot action
                match app.game.get_bot_action(bot_player) {
                    Ok(bot_action) => {
                        let action_str = match &bot_action {
                            GameAction::Fold => "fold".to_string(),
                            GameAction::Call => "call".to_string(),
                            GameAction::Check => "check".to_string(),
                            GameAction::Raise(amount) => format!("raise {}", amount),
                        };
                        
                        // Not used but kept for future reference if needed
                        let _bot_position = get_player_position(&app.game, app.game.current_player_idx);
                        // Simplify message to reduce overflow risk
                        app.messages.push(format!("{} {}s.", bot_player.name, action_str));
                        app.game.perform_action(bot_action);
                        
                        let game_continues = app.game.next_player();
                        
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
                            // If next player is a bot, set thinking time (shorter for better UX)
                            app.bot_thinking = true;
                            app.bot_think_until = std::time::Instant::now() + 
                                std::time::Duration::from_millis(rand::thread_rng().gen_range(800..1500));
                        }
                        
                        // Safety check to prevent infinite loop
                        if app.game.last_action_count > 15 {
                            app.messages.push("Round ending (action limit).".to_string());
                            let (winner_idx, winnings, hand_type) = app.game.determine_winner();
                            // Use minimum winnings display here too with shorter format
                            let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
                            // Add to game stats
                            let human_idx = app.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
                            let human_player = &app.game.players[human_idx];
                            let profit = human_player.chips as i32 - app.player_starting_chips as i32;
                            app.game_stats.push(profit);
                            let total_profit = app.game_stats.iter().sum::<i32>();
                            
                            app.messages.push(format!("{} wins ${} with {}! Your total profit: ${}", 
                                                    app.game.players[winner_idx].name, display_winnings, hand_type, total_profit));
                            app.game.last_action_count = 0;
                            app.game_active = false;
                            app.messages.push("Press 'd' to deal a new hand.".to_string());
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
        
        // Draw the UI
        terminal.draw(|f| {
            // Create layout - use more space efficiently
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(7),   // Game info (expanded)
                    Constraint::Length(3),   // Community cards
                    Constraint::Length(3),   // Player hand
                    Constraint::Min(10),     // Messages (expanded)
                    Constraint::Length(3),   // Input
                ].as_ref())
                .split(f.size());
            
            // Show whose turn it is - keep brief for small screens 
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
               app.messages.last().map_or(true, |msg| !msg.contains("Your turn now")) {
                // Only add this message if we haven't added it recently (avoid duplicates)
                if app.messages.len() < 2 || !app.messages[app.messages.len() - 2].contains("Your turn now") {
                    app.messages.push("Your turn now. Choose action: [c]all, [k]heck, [f]old, or [r]aise.".to_string());
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

            // Style the pot amount with color based on size
            let pot_style = if app.game.pot > 100 {
                Style::default().fg(tui::style::Color::Yellow)
            } else if app.game.pot > 50 {
                Style::default().fg(tui::style::Color::Green)
            } else {
                Style::default()
            };
            
            // Calculate available width to ensure no overflow
            let total_width = f.size().width as usize - 4; // Account for borders
            let truncate_large = total_width < 70; // If screen is narrow, use shorter format
            
            let game_info = Paragraph::new(vec![
                // Row 1: Basic game stats (with potential truncation)
                
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
                    ), Style::default().fg(tui::style::Color::Cyan)),
                    Span::raw("| "),
                    Span::raw(if truncate_large { "Bet: " } else { "Current Bet: " }),
                    Span::styled(format!("${}", 
                        app.game.players.iter()
                            .map(|p| p.current_bet)
                            .max()
                            .unwrap_or(0)
                    ), Style::default().fg(tui::style::Color::Yellow)),
                ]),
                // Row 2: Round information (with potential truncation)
                Line::from(vec![
                    Span::raw("Round: "),
                    Span::styled(format!("{:?}", app.game.round), Style::default().fg(tui::style::Color::Green)),
                    Span::raw(" | Active Players: "),
                    Span::styled(format!("{} ({} bots)", active_players, app.game.players.len() - 1), 
                                Style::default().fg(tui::style::Color::Blue)),
                    Span::raw(" | "),
                    Span::raw(if truncate_large { "Pos: " } else { "Position: " }),
                    Span::styled(
                        // Truncate position name if too long
                        if human_position.len() > 15 && truncate_large {
                            format!("{}...", &human_position[0..12])
                        } else {
                            human_position
                        }, 
                        Style::default().fg(tui::style::Color::Cyan)
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
                        Style::default().fg(tui::style::Color::Yellow)
                    ),
                    Span::raw(" | SB: "),
                    Span::styled(
                        // Truncate SB name if too long
                        if app.game.players[app.game.small_blind_idx].name.len() > 10 && truncate_large {
                            format!("{}...", &app.game.players[app.game.small_blind_idx].name[0..7])
                        } else {
                            app.game.players[app.game.small_blind_idx].name.clone()
                        },
                        Style::default().fg(tui::style::Color::Yellow)
                    ),
                    Span::raw(" | BB: "),
                    Span::styled(
                        // Truncate BB name if too long
                        if app.game.players[app.game.big_blind_idx].name.len() > 10 && truncate_large {
                            format!("{}...", &app.game.players[app.game.big_blind_idx].name[0..7])
                        } else {
                            app.game.players[app.game.big_blind_idx].name.clone()
                        },
                        Style::default().fg(tui::style::Color::Yellow)
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
                        Style::default().fg(tui::style::Color::White))
                ]),
                // Row 5: Game stats or turn info (with truncation for long texts)
                Line::from(vec![
                    Span::styled("► ", Style::default().fg(tui::style::Color::Green)),
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
                        Style::default().fg(tui::style::Color::Cyan))
                ]),
                // Row 6: Last result and game status (with truncation)
                Line::from(vec![
                    Span::styled(
                        if result_display.len() > 35 {
                            format!("{}...", &result_display[0..32]) 
                        } else {
                            result_display.to_string()
                        },
                        Style::default().fg(tui::style::Color::Green)
                    ),
                    Span::raw("   "),
                    Span::styled(
                        if game_status.len() > 35 {
                            format!("{}...", &game_status[0..32])
                        } else {
                            game_status.to_string()
                        },
                        Style::default().fg(tui::style::Color::Yellow)
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
            
            // Messages - improve formatting and handle small screens
            // Calculate max message width with safety margin to prevent overflow
            let max_msg_width = if f.size().width > 10 { f.size().width as usize - 8 } else { 2 };
            
            // Keep more history and allow scrolling
            // Display all messages without limit for scrolling
            let messages: Vec<ListItem> = app.messages.iter()
                .map(|m| {
                    // More aggressive truncation for messages
                    let display_msg = if m.len() > max_msg_width {
                        // Ensure we don't go out of bounds with very small windows
                        let end_pos = if max_msg_width > 5 { max_msg_width - 3 } else { 2 };
                        format!("{}...", &m[0..end_pos])
                    } else {
                        m.clone()
                    };
                    
                    // Use appropriate styling for different message types
                    if m.contains("wins") || m.contains("won") {
                        ListItem::new(vec![Line::from(vec![
                            Span::styled(display_msg, Style::default().fg(tui::style::Color::Green))
                        ])])
                    } else if m.contains("lost") || m.contains("error") || m.contains("fold") {
                        ListItem::new(vec![Line::from(vec![
                            Span::styled(display_msg, Style::default().fg(tui::style::Color::Red))
                        ])])
                    } else if m.contains("Your") || m.contains("You") {
                        ListItem::new(vec![Line::from(vec![
                            Span::styled(display_msg, Style::default().fg(tui::style::Color::Cyan))
                        ])])
                    } else if m.contains("thinking") {
                        ListItem::new(vec![Line::from(vec![
                            Span::styled(display_msg, Style::default().fg(tui::style::Color::Yellow))
                        ])])
                    } else {
                        ListItem::new(vec![Line::from(vec![Span::raw(display_msg)])])
                    }
                })
                .collect();
            
            // Create scrollable list using StatefulList
            let messages_state = &mut ListState::default();
            
            // Auto-scroll to bottom if not manually scrolled up,
            // otherwise keep user's scroll position
            if !messages.is_empty() {
                // If user hasn't scrolled up manually or we're adding new messages
                if app.message_scroll_pos == 0 || app.message_scroll_pos >= messages.len() - 2 {
                    // Auto-scroll to bottom
                    app.message_scroll_pos = messages.len() - 1;
                }
                
                // This ensures the selected item is always visible
                messages_state.select(Some(app.message_scroll_pos.min(messages.len() - 1)));
            }
            
            // Create a scrollable style with visual indication
            let messages_widget = List::new(messages)
                .block(Block::default()
                    .title(format!("Game Log (Scrollable - {}/{})", 
                                   app.message_scroll_pos + 1, 
                                   app.messages.len()))
                    .borders(Borders::ALL))
                .highlight_style(Style::default()
                    .fg(tui::style::Color::Yellow)
                    .add_modifier(Modifier::BOLD));
            
            // Render with state to enable scrolling
            f.render_stateful_widget(messages_widget, chunks[3], messages_state);
            
            // Input with enhanced info about available commands including scroll hints
            let input_title = if app.input_mode == InputMode::PlayerName {
                "Input [Enter name, press 'n' to confirm]"
            } else if app.game_active && !app.bot_thinking && !app.game.players[app.game.current_player_idx].is_bot {
                "Input [r:raise c:call k:check f:fold s:stop n:set-name q:quit]"
            } else if app.bot_thinking {
                "Input [WAITING...]"
            } else if !app.game_active {
                "Input [d:deal n:set-name q:quit]"
            } else {
                "Input [WAITING FOR YOUR TURN...]"
            };
            
            // Truncate input if it gets too long
            let display_input = if app.input.len() > f.size().width as usize - 6 {
                format!("{}...", &app.input[0..(f.size().width as usize - 9)])
            } else {
                app.input.clone()
            };
            
            // Also truncate the title if needed
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
        
        // Handle user input
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = crossterm::event::read()? {
                app.on_key(key.code);
                if app.should_quit {
                    break;
                }
            }
        }
    }
    
    // Restore terminal
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    
    Ok(())
}