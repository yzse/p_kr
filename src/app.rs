use std::time::Instant;
use crossterm::event::KeyCode;
use rand::Rng;
use crate::game::{Game, GameAction, BotDifficulty, Round};
use crate::util;
use crate::util::get_player_position;

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,   // Regular game input
    PlayerName, // Entering player name
}

pub struct App {
    pub game: Game,
    pub input: String,
    pub messages: Vec<String>,
    pub should_quit: bool,
    pub player_starting_chips: u32, // To track wins/losses
    pub round_results: Option<(String, i32)>, // (Winner name, player profit/loss)
    pub game_stats: Vec<i32>, // Track player profits across multiple rounds
    pub bot_thinking: bool,         // To simulate bot thinking time
    pub bot_think_until: Instant, // When bot should finish "thinking"
    pub game_active: bool,          // Whether a game is currently in progress
    pub message_scroll_pos: usize,  // Position in message history for scrolling
    pub input_mode: InputMode,      // Current input mode (raise amount or player name)
}

impl App {
    pub fn new(api_key: Option<String>, player_name: String) -> Self {
        // Starting chips amount
        let starting_chips = 200;
        
        // Set up a game with 1 human player and 8 bots (total 9 players)
        let game = Game::new(1, 8, BotDifficulty::Medium, starting_chips, api_key, player_name);
        
        // Create initial instructions
        let initial_messages = vec![
            "Press 'd' to deal a new hand, 'q' to quit.".to_string(),
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
            bot_think_until: Instant::now(),
            game_active: false,
            message_scroll_pos: 4, // Start at bottom of instructions
            input_mode: InputMode::Normal
        }
    }
    
    pub fn on_key(&mut self, key: KeyCode) {
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
                        self.messages.push("\nNew hand dealt.".to_string());
                        
                        // Add clear messages about blinds
                        let sb_name = self.game.players[self.game.small_blind_idx].name.clone();
                        let bb_name = self.game.players[self.game.big_blind_idx].name.clone();
                        
                        // Get positions for display (currently unused but kept for future enhancements)
                        let _small_blind_pos = util::get_player_position(&self.game, self.game.small_blind_idx);
                        let _big_blind_pos = util::get_player_position(&self.game, self.game.big_blind_idx);
                        
                        // Add clear blind posts
                        self.messages.push(format!("{} in Small Blind (SB) position posts ${}", 
                                                  sb_name, self.game.min_bet / 2));
                        self.messages.push(format!("{} in Big Blind (BB) position posts ${}", 
                                                  bb_name, self.game.min_bet));
                        
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
                                // Check if there's a bet to call
                                let highest_bet = self.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
                                let player_current_bet = self.game.players[self.game.current_player_idx].current_bet;
                                
                                if highest_bet <= player_current_bet {
                                    self.messages.push("No bet to call - action changed to check.".to_string());
                                }
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
                                // Check if there's a bet to call (can't check if there is)
                                let highest_bet = self.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
                                let player_current_bet = self.game.players[self.game.current_player_idx].current_bet;
                                
                                if highest_bet > player_current_bet {
                                    self.messages.push("There's a bet - action changed to call.".to_string());
                                }
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
    
    pub fn print_game_stats(&mut self) {
        if !self.game_stats.is_empty() {
            let total_profit = self.game_stats.iter().sum::<i32>();
            
            // Get the human's current chip stack
            let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
            let current_chips = self.game.players[human_idx].chips;
            
            // Calculate profit for the current round
            let current_round_profit = if !self.game_stats.is_empty() {
                self.game_stats.last().unwrap()
            } else {
                &0
            };
            
            // Format the round profits
            let round_profits = self.game_stats.iter()
                .enumerate()
                .map(|(i, profit)| format!("R{}: ${}{}", i+1, if *profit >= 0 {""} else {"-"}, profit.abs()))
                .collect::<Vec<_>>()
                .join(". ");
            
            // Show detailed stats
            self.messages.push(format!(
                "Game stats: {} rounds played. Current round profit: ${}{}. Total profit: ${}. Current chips: ${}", 
                self.game_stats.len(), 
                if *current_round_profit >= 0 { "" } else { "-" },
                current_round_profit.abs(),
                total_profit,
                current_chips
            ));
            
            // Show round-by-round profits
            self.messages.push(format!("Round profits: {}", round_profits));
        } else {
            self.messages.push("Game stats: No rounds played yet.".to_string());
        }
    }
    
    pub fn handle_player_action(&mut self, action: GameAction) {
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
            
            // Show all players' hands who haven't folded
            self.messages.push("--- SHOWDOWN: Players reveal their hands ---".to_string());
            for (idx, player) in self.game.players.iter().enumerate() {
                if !player.folded && player.hand.len() >= 2 {
                    let hand_str = player.hand.iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                        
                    let position = get_player_position(&self.game, idx);
                    
                    if player.is_bot {
                        self.messages.push(format!("{} ({}) shows: {}", player.name, position, hand_str));
                    } else {
                        self.messages.push(format!("You ({}) show: {}", position, hand_str));
                    }
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
            let community_display = if !self.game.community_cards.is_empty() {
                let cards = self.game.community_cards.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(" (with community cards: {})", cards)
            } else {
                "".to_string()
            };
            
            // Display results in message log with more detail
            let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
            self.messages.push(format!("Round over! {} wins ${} chips with {}{}!", 
                                      self.game.players[winner_idx].name, display_winnings, 
                                      hand_type, community_display));
            
            // Add explanation if available
            if !hand_explanation.is_empty() {
                self.messages.push(format!("Hand info: {}", hand_explanation));
            }
            
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
        // We don't need this anymore since we use actual_action_str
        // Just keeping a placeholder to ensure proper code flow
        let _original_intent = &action;
        
        // Get the highest bet for terminology
        let highest_bet = self.game.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        let is_first_bet = highest_bet == 0 || highest_bet == self.game.min_bet; // Consider BB as not a "bet"
        
        // Save the original action type for comparison
        let original_action_type = match &action {
            GameAction::Fold => 0,
            GameAction::Call => 1,
            GameAction::Check => 2,
            GameAction::Raise(_) => 3,
        };
        
        // Perform the action and get the actual action performed
        let actual_action = self.game.perform_action(action.clone());
        
        // Update action string based on what was actually performed
        let actual_action_str = match &actual_action.0 {
            GameAction::Fold => "fold".to_string(),
            GameAction::Call => "call".to_string(),
            GameAction::Check => "check".to_string(),
            GameAction::Raise(amount) => {
                // More accurate bet/raise distinction
                if is_first_bet && self.game.round != Round::PreFlop {
                    // This is a bet, not a raise (but in PreFlop, it's still a raise)
                    if let Some(total) = actual_action.1 {
                        format!("bet {}", total)
                    } else {
                        format!("bet {}", amount)
                    }
                } else {
                    // This is a raise
                    if let Some(total) = actual_action.1 {
                        format!("raise to {}", total)
                    } else {
                        format!("raise by {}", amount)
                    }
                }
            },
        };
        
        // Check if the actual action type is different from the original
        let actual_type = match &actual_action.0 {
            GameAction::Fold => 0,
            GameAction::Call => 1,
            GameAction::Check => 2,
            GameAction::Raise(_) => 3,
        };
        
        // If the actual action is different from requested, let player know
        if actual_type != original_action_type {
            // For a call converted to check
            if matches!(action, GameAction::Call) && matches!(actual_action.0, GameAction::Check) {
                self.messages.push("No bet to call - action changed to check.".to_string());
            }
            // For a check converted to call
            else if matches!(action, GameAction::Check) && matches!(actual_action.0, GameAction::Call) {
                self.messages.push("There's a bet - action changed to call.".to_string());
            }
            // For a raise converted to check or call
            else if matches!(action, GameAction::Raise(_)) && 
                   (matches!(actual_action.0, GameAction::Check) || 
                    matches!(actual_action.0, GameAction::Call)) {
                self.messages.push("Not enough chips for minimum raise - action changed.".to_string());
            }
        }
        
        // Log the player's action
        self.messages.push(format!("You (in {} position) {}.", player_position, actual_action_str));
        
        // Get player index (for logging chip changes)
        let human_idx = self.game.players.iter().position(|p| !p.is_bot).unwrap_or(0);
        
        // Get the contribution amount for logging
        let player_contribution = match &actual_action.0 {
            GameAction::Call | GameAction::Raise(_) => {
                // For calls or raises, the contribution is how much the player added to the pot
                self.game.player_contributions_this_round[current_player_idx] -
                    (if let Some(previous_contribution) = self.game.player_contributions_this_round.get(current_player_idx) {
                        *previous_contribution
                    } else {
                        0
                    })
            },
            _ => 0 // No contribution for fold or check
        };
        
        // Calculate old pot for logging
        let old_pot = if player_contribution > 0 {
            self.game.pot - player_contribution
        } else {
            self.game.pot
        };
        
        // Log pot increase (only if it changed)
        if old_pot < self.game.pot {
            self.messages.push(format!("Pot increased from ${} to ${}.", old_pot, self.game.pot));
        }
        
        // Log player chip changes if this is the human player and they're contributing chips
        // Only show the message for calls and raises, not for folds or checks
        if current_player_idx == human_idx {
            let chips_before = self.player_starting_chips;
            let chips_now = self.game.players[human_idx].chips;
            let actual_action_type = match &actual_action.0 {
                GameAction::Call | GameAction::Raise(_) => true,
                _ => false
            };
            
            // Only show chip change message if chips actually changed AND the action was a call or raise
            if chips_before != chips_now && actual_action_type {
                if chips_before > chips_now {
                    self.messages.push(format!("Your chips decreased from ${} to ${}.", 
                                             chips_before, chips_now));
                } else {
                    self.messages.push(format!("Your chips increased from ${} to ${}.", 
                                             chips_before, chips_now));
                }
            }
        }
        
        // Get the current round before moving to next player
        let current_round = self.game.round;
        
        // Move to next player
        let game_continues = self.game.next_player();
        
        // Check if round changed (to make turn transitions more visible)
        let new_round = self.game.round;
        if new_round != current_round {
            // Add a message about round transition
            match new_round {
                Round::Flop => self.messages.push("--- Moving to FLOP round (first 3 community cards) ---".to_string()),
                Round::Turn => self.messages.push("--- Moving to TURN round (4th community card) ---".to_string()),
                Round::River => self.messages.push("--- Moving to RIVER round (final community card) ---".to_string()),
                Round::Showdown => {
                    self.messages.push("--- Moving to SHOWDOWN (comparing hands) ---".to_string());
                    
                    // In Showdown, we should immediately determine the winner
                    // This eliminates the need for the player to act again
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
                    let community_display = if !self.game.community_cards.is_empty() {
                        let cards = self.game.community_cards.iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        format!(" (with community cards: {})", cards)
                    } else {
                        "".to_string()
                    };
                    
                    // Display results in message log with more detail
                    let display_winnings = if winnings == 0 { 10 } else { winnings }; // Minimum 10 chips
                    self.messages.push(format!("Round over! {} wins ${} chips with {}{}!", 
                                            self.game.players[winner_idx].name, display_winnings, 
                                            hand_type, community_display));
                    
                    // Add explanation if available
                    if !hand_explanation.is_empty() {
                        self.messages.push(format!("Hand info: {}", hand_explanation));
                    }
                    
                    if winner_idx == human_idx {
                        self.messages.push(format!("You won this hand! Your profit: ${}. Total: ${}", profit.abs(), total_profit));
                    } else {
                        self.messages.push(format!("You lost this hand. Your loss: ${}. Total: ${}", profit.abs(), total_profit));
                    }
                    
                    // Add a small delay to ensure UI updates correctly
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    // Print game stats
                    self.print_game_stats();
                    
                    // End the game
                    self.game_active = false;
                    self.messages.push("Press 'd' to deal a new hand.".to_string());
                    self.messages.push("".to_string()); // Add empty line between rounds
                    
                    // Ensure the message scroll position is updated to show the latest messages
                    self.message_scroll_pos = self.messages.len().saturating_sub(1);
                    return;
                },
                _ => {}
            }
        }
        
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
            let community_display = if !self.game.community_cards.is_empty() {
                let cards = self.game.community_cards.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                format!(" (with community cards: {})", cards)
            } else {
                "".to_string()
            };
            
            // Display results in message log with more detail
            self.messages.push(format!("Round over! {} wins ${} chips with {}{}!", 
                                      self.game.players[winner_idx].name, winnings, 
                                      hand_type, community_display));
            
            // Add explanation if available
            if !hand_explanation.is_empty() {
                self.messages.push(format!("Hand info: {}", hand_explanation));
            }
            
            if winner_idx == human_idx {
                self.messages.push(format!("You won this hand! Your profit: ${}.", profit.abs()));
            } else {
                self.messages.push(format!("You lost this hand. Your loss: ${}.", profit.abs()));
            }
            
            // Print game stats
            self.print_game_stats();
            return;
        }
        
        // If next player is a bot, set up realistic thinking time
        if self.game.players[self.game.current_player_idx].is_bot {
            self.bot_thinking = true;
            self.bot_think_until = Instant::now() + 
                std::time::Duration::from_millis(rand::thread_rng().gen_range(1500..3000));
        }
    }
}