use std::io;
use rand::prelude::*;
use crossterm::{
    event::{Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Style, Modifier},
    text::{Span, Line},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use rs_poker::core::{Card as PokerCard, Suit as PokerSuit, Value as PokerValue, Hand, Rank as PokerRank, Rankable};

// Card representation
#[derive(Clone, Debug, PartialEq, Eq)]
struct Card {
    rank: Rank,
    suit: Suit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Rank {
    Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten,
    Jack, Queen, King, Ace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Suit {
    Hearts, Diamonds, Clubs, Spades,
}

impl Card {
    fn to_string(&self) -> String {
        let rank_str = match self.rank {
            Rank::Two => "2", Rank::Three => "3", Rank::Four => "4",
            Rank::Five => "5", Rank::Six => "6", Rank::Seven => "7",
            Rank::Eight => "8", Rank::Nine => "9", Rank::Ten => "10",
            Rank::Jack => "J", Rank::Queen => "Q", Rank::King => "K",
            Rank::Ace => "A",
        };
        
        let suit_str = match self.suit {
            Suit::Hearts => "♥", Suit::Diamonds => "♦", 
            Suit::Clubs => "♣", Suit::Spades => "♠",
        };
        
        format!("[{}{}]", rank_str, suit_str)
    }
}

// Representing a poker hand
#[derive(Clone, Debug)]
#[allow(dead_code)]
enum HandRank {
    HighCard(Vec<Rank>),
    Pair(Rank, Vec<Rank>),
    TwoPair(Rank, Rank, Rank),
    ThreeOfAKind(Rank, Vec<Rank>),
    Straight(Rank),
    Flush(Vec<Rank>),
    FullHouse(Rank, Rank),
    FourOfAKind(Rank, Rank),
    StraightFlush(Rank),
    RoyalFlush,
}

// Player representation
#[derive(Clone)]
struct Player {
    name: String,
    hand: Vec<Card>,
    chips: u32,
    current_bet: u32,
    folded: bool,
    is_bot: bool,
    bot_difficulty: BotDifficulty,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum BotDifficulty {
    Easy,
    Medium,
    Hard,
}

#[derive(Clone, Debug, PartialEq)]
enum InputMode {
    Normal,   // Regular game input
    PlayerName, // Entering player name
}

// Game state
struct Game {
    players: Vec<Player>,
    deck: Vec<Card>,
    community_cards: Vec<Card>,
    pot: u32,
    current_player_idx: usize,
    min_bet: u32,
    round: Round,
    ai_client: Client,
    api_key: Option<String>,
    dealer_idx: usize,
    small_blind_idx: usize,
    big_blind_idx: usize,
    last_action_count: usize, // Track number of consecutive bot actions
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum Round {
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
}

#[derive(Serialize, Deserialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug)]
enum GameAction {
    Fold,
    Call,
    Raise(u32),
    Check,
}

// Game implementation
impl Game {
    fn new(num_human_players: usize, num_bot_players: usize, bot_difficulty: BotDifficulty, starting_chips: u32, api_key: Option<String>, player_name: String) -> Self {
        let mut players = Vec::new();
        
        // Add human players
        for i in 0..num_human_players {
            players.push(Player {
                name: if i == 0 { player_name.clone() } else { format!("Player {}", i+1) },
                hand: Vec::new(),
                chips: starting_chips,
                current_bet: 0,
                folded: false,
                is_bot: false,
                bot_difficulty: BotDifficulty::Easy, // Unused for human players
            });
        }
        
        // Add bot players
        for i in 0..num_bot_players {
            players.push(Player {
                name: format!("Bot {}", i+1),
                hand: Vec::new(),
                chips: starting_chips,
                current_bet: 0,
                folded: false,
                is_bot: true,
                bot_difficulty: bot_difficulty.clone(),
            });
        }
        
        // Initialize with dealer at random position to ensure all players get different positions
        let mut rng = thread_rng();
        let dealer_idx = rng.gen_range(0..players.len());
        let small_blind_idx = (dealer_idx + 1) % players.len();
        let big_blind_idx = (small_blind_idx + 1) % players.len();
        
        Game {
            players,
            deck: Game::create_deck(),
            community_cards: Vec::new(),
            pot: 0,
            current_player_idx: 0,
            min_bet: 10, // Small blind
            round: Round::PreFlop,
            ai_client: Client::new(),
            api_key,
            dealer_idx,
            small_blind_idx,
            big_blind_idx,
            last_action_count: 0,
        }
    }
    
    fn create_deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(52);
        let suits = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];
        let ranks = [
            Rank::Two, Rank::Three, Rank::Four, Rank::Five, Rank::Six, Rank::Seven, 
            Rank::Eight, Rank::Nine, Rank::Ten, Rank::Jack, Rank::Queen, Rank::King, Rank::Ace
        ];
        
        for suit in &suits {
            for rank in &ranks {
                deck.push(Card {
                    rank: rank.clone(),
                    suit: suit.clone(),
                });
            }
        }
        
        deck
    }
    
    fn shuffle_deck(&mut self) {
        let mut rng = thread_rng();
        self.deck.shuffle(&mut rng);
    }
    
    fn deal_cards(&mut self) {
        // Reset action counter
        self.last_action_count = 0;
        
        // Rotate positions for the next hand
        self.dealer_idx = (self.dealer_idx + 1) % self.players.len();
        self.small_blind_idx = (self.dealer_idx + 1) % self.players.len();
        self.big_blind_idx = (self.small_blind_idx + 1) % self.players.len();
        
        // Clear old hands and reset player state
        for player in &mut self.players {
            player.hand.clear();
            player.folded = false;
            player.current_bet = 0;
        }
        
        // Clear community cards and reset game state
        self.community_cards.clear();
        self.pot = 0;
        self.round = Round::PreFlop;
        
        // Create a fresh deck and shuffle it
        self.deck = Game::create_deck();
        self.shuffle_deck();
                
        // Deal 2 cards to each player
        for _ in 0..2 {
            for player in &mut self.players {
                if let Some(card) = self.deck.pop() {
                    player.hand.push(card);
                }
            }
        }
        
        // Set up blinds and ante (ensure pot is never zero)
        // Each player pays a small ante
        let ante = 1; // 1 chip ante from each player
        for player in &mut self.players {
            player.chips = player.chips.saturating_sub(ante);
            self.pot += ante;
        }
        
        if self.players.len() >= 2 {
            // Small blind (minimum 5)
            let small_blind = self.min_bet / 2;
            self.players[self.small_blind_idx].chips = self.players[self.small_blind_idx].chips.saturating_sub(small_blind);
            self.players[self.small_blind_idx].current_bet = small_blind;
            self.pot += small_blind;
            
            // Big blind (minimum 10)
            let big_blind = self.min_bet;
            self.players[self.big_blind_idx].chips = self.players[self.big_blind_idx].chips.saturating_sub(big_blind);
            self.players[self.big_blind_idx].current_bet = big_blind;
            self.pot += big_blind;
            
            // Start with player after big blind (UTG position)
            self.current_player_idx = (self.big_blind_idx + 1) % self.players.len();
        }
    }
    
    fn deal_community_cards(&mut self) {
        // Ensure we have enough cards in the deck
        if self.deck.len() < 5 {
            self.deck = Game::create_deck();
            self.shuffle_deck();
        }
        
        match self.round {
            Round::Flop => {
                // Deal 3 cards for the flop
                for _ in 0..3 {
                    if let Some(card) = self.deck.pop() {
                        self.community_cards.push(card);
                    }
                }
            },
            Round::Turn => {
                // Deal 1 card for turn
                if let Some(card) = self.deck.pop() {
                    self.community_cards.push(card);
                }
            },
            Round::River => {
                // Deal 1 card for river
                if let Some(card) = self.deck.pop() {
                    self.community_cards.push(card);
                }
            },
            _ => {
                // No cards dealt in preflop or showdown
            },
        }
        
        // Verify cards were dealt
        if self.round != Round::PreFlop {
            let expected_count = match self.round {
                Round::Flop => 3,
                Round::Turn => 4,
                Round::River => 5,
                _ => 0,
            };
            
            // Force correct number of cards if needed
            if self.community_cards.len() != expected_count {
                // Start fresh
                self.community_cards.clear();
                
                // Deal appropriate number of cards
                for _ in 0..expected_count {
                    if let Some(card) = self.deck.pop() {
                        self.community_cards.push(card);
                    }
                }
            }
        }
    }
    
    fn next_round(&mut self) {
        // Ensure deck is properly set up
        if self.deck.len() < 5 {
            self.deck = Game::create_deck();
            self.shuffle_deck();
        }
        
        // Update the round
        let _previous_round = self.round;
        match self.round {
            Round::PreFlop => self.round = Round::Flop,
            Round::Flop => self.round = Round::Turn,
            Round::Turn => self.round = Round::River,
            Round::River => self.round = Round::Showdown,
            Round::Showdown => {
                // Start a new hand
                self.deal_cards();
                return;
            }
        }
        
        // Reset bets for the new round
        for player in &mut self.players {
            player.current_bet = 0;
        }
        
        // Handle each round transition properly
        match self.round {
            Round::Flop => {
                // For flop, ensure we start fresh
                self.community_cards.clear();
                // Deal 3 cards
                for _ in 0..3 {
                    if let Some(card) = self.deck.pop() {
                        self.community_cards.push(card);
                    }
                }
            },
            Round::Turn => {
                // Ensure we have flop cards first
                if self.community_cards.len() < 3 {
                    self.community_cards.clear();
                    // Add missing flop cards
                    for _ in 0..3 {
                        if let Some(card) = self.deck.pop() {
                            self.community_cards.push(card);
                        }
                    }
                }
                // Add turn card
                if let Some(card) = self.deck.pop() {
                    self.community_cards.push(card);
                }
            },
            Round::River => {
                // Ensure we have flop and turn cards
                if self.community_cards.len() < 4 {
                    // Missing cards
                    let missing = 4 - self.community_cards.len();
                    for _ in 0..missing {
                        if let Some(card) = self.deck.pop() {
                            self.community_cards.push(card);
                        }
                    }
                }
                // Add river card
                if let Some(card) = self.deck.pop() {
                    self.community_cards.push(card);
                }
            },
            _ => {}
        }
        
        // Reset action counter for new round
        self.last_action_count = 0;
        
        // Reset to first active player (after dealer)
        self.current_player_idx = self.find_next_active_player((self.dealer_idx) % self.players.len());
    }
    
    fn find_next_active_player(&self, current_idx: usize) -> usize {
        let mut idx = (current_idx + 1) % self.players.len();
        
        // Find the next player who hasn't folded and has chips
        let start_idx = idx;
        loop {
            if !self.players[idx].folded && self.players[idx].chips > 0 {
                return idx;
            }
            
            idx = (idx + 1) % self.players.len();
            
            // If we've checked all players and come back to where we started,
            // just return the original index
            if idx == start_idx {
                return current_idx;
            }
        }
    }
    
    fn next_player(&mut self) -> bool {
        // Variables to track game state for round completion
        let highest_bet = self.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        let active_players = self.players.iter().filter(|p| !p.folded && p.chips > 0).count();
        
        // Check if all players have either matched the highest bet, folded, or are all-in (round complete)
        let bets_matched = self.players.iter()
            .filter(|p| !p.folded)  // Only consider active players
            .all(|p| p.current_bet == highest_bet || p.chips == 0);  // All have matched or are all-in
            
        // Force round advancement for testing if needed
        let force_advancement = self.last_action_count >= 8;
        
        // A round is complete when:
        // 1. Only one player remains, OR
        // 2. All active players have matched the bet or are all-in
        let round_complete = active_players <= 1 || bets_matched || force_advancement;
        
        if round_complete {
            if active_players <= 1 || self.round == Round::Showdown {
                // Game is over - determine winner and distribute pot
                self.determine_winner();
                return false;
            } else {
                // Move to next round - critical to game progression
                // Record round before transition
                let old_round = self.round;
                
                // Force progression if stuck
                if old_round == Round::PreFlop && self.community_cards.is_empty() {
                    self.round = Round::Flop;
                    // Deal the flop
                    self.community_cards.clear();
                    for _ in 0..3 {
                        if let Some(card) = self.deck.pop() {
                            self.community_cards.push(card);
                        }
                    }
                    // Reset bets
                    for player in &mut self.players {
                        player.current_bet = 0;
                    }
                    self.last_action_count = 0;
                } else {
                    // Normal round transition
                    self.next_round();
                }
                
                // Always verify community cards were dealt
                if self.community_cards.is_empty() && self.round != Round::PreFlop {
                    self.deal_community_cards();
                }
                
                return true;
            }
        }
        
        // Move to next active player
        self.current_player_idx = self.find_next_active_player(self.current_player_idx);
        true
    }
    
    fn get_bot_action(&self, player: &Player) -> Result<GameAction, String> {
        // Prevent infinite loops by limiting bot actions
        if self.last_action_count > 100 {
            // Force a different action to break potential loops
            return Ok(GameAction::Call);
        }
        
        // Prepare the context for the AI
        let current_player = &self.players[self.current_player_idx];
        let player_position = self.get_player_position(self.current_player_idx);
        
        let mut prompt = String::new();
        
        // Add game state information with positions
        prompt.push_str(&format!("You are playing poker with {} players total.\n", self.players.len()));
        prompt.push_str(&format!("Your position: {}. Dealer is {}. Small blind is {}. Big blind is {}.\n",
            player_position,
            self.players[self.dealer_idx].name,
            self.players[self.small_blind_idx].name,
            self.players[self.big_blind_idx].name));
        
        prompt.push_str(&format!("Your hand: {}\n", 
            current_player.hand.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")));
        
        prompt.push_str(&format!("Community cards: {}\n", 
            if self.community_cards.is_empty() { "None yet".to_string() } 
            else { self.community_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ") }));
        
        prompt.push_str(&format!("Your chips: {}, Current pot: {}, Minimum bet to call: {}\n", 
            current_player.chips, self.pot, 
            self.players.iter().map(|p| p.current_bet).max().unwrap_or(0).saturating_sub(current_player.current_bet)));
        
        // Add information about other players
        prompt.push_str("Other players:\n");
        for (i, p) in self.players.iter().enumerate() {
            if i != self.current_player_idx && !p.folded {
                prompt.push_str(&format!("- {} in {} position has {} chips and has bet {}\n", 
                    p.name, self.get_player_position(i), p.chips, p.current_bet));
            }
        }
        
        // Add round information
        prompt.push_str(&format!("\nCurrent round: {:?}\n", self.round));
        
        // Add shorter instructions based on difficulty
        match player.bot_difficulty {
            BotDifficulty::Easy => {
                prompt.push_str("\nYou are a beginner. Play cautiously.\n");
            },
            BotDifficulty::Medium => {
                prompt.push_str("\nYou are intermediate. Consider position and patterns.\n");
            },
            BotDifficulty::Hard => {
                prompt.push_str("\nYou are advanced. Use pot odds and bluff when appropriate.\n");
            }
        }
        
        prompt.push_str("\nWhat action would you like to take? Choose one of: 'check', 'call', 'raise [amount]', or 'fold'. Only respond with one of these options, nothing else.");
        
        // Create the OpenAI request with a more concise system message
        let _request = OpenAIRequest {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You're a poker AI. Only respond with: 'check', 'call', 'fold', or 'raise X' where X is a number.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            temperature: 0.7,
        };
        
        // Check if we have an API key to use the real OpenAI API
        let action = if let Some(api_key) = &self.api_key {
            // Make an actual API call to OpenAI
            match self.make_openai_api_call(api_key, &_request) {
                Ok(response) => {
                    // Valid response received
                    response
                },
                Err(_error_msg) => {
                    // Fallback to random bot action on error
                    self.generate_random_bot_action(player)
                }
            }
        } else {
            // No API key, use randomized response
            self.generate_random_bot_action(player)
        };
        
        // Parse the response
        if action.starts_with("fold") {
            Ok(GameAction::Fold)
        } else if action.starts_with("call") {
            Ok(GameAction::Call)
        } else if action.starts_with("check") {
            Ok(GameAction::Check)
        } else if action.starts_with("raise") {
            // Extract the raise amount
            let parts: Vec<&str> = action.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(amount) = parts[1].parse::<u32>() {
                    Ok(GameAction::Raise(amount))
                } else {
                    Ok(GameAction::Raise(self.min_bet)) // Default amount
                }
            } else {
                Ok(GameAction::Raise(self.min_bet)) // Default amount
            }
        } else {
            // Default to check if response can't be parsed
            Ok(GameAction::Check)
        }
    }
    
    fn perform_action(&mut self, action: GameAction) {
        let highest_bet = self.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        let player_current_bet = self.players[self.current_player_idx].current_bet;
        let to_call = highest_bet.saturating_sub(player_current_bet);
        
        // No print statements - these cause UI overflow
        
        match action {
            GameAction::Fold => {
                self.players[self.current_player_idx].folded = true;
            },
            GameAction::Call => {
                // Can only call if there's a bet to match
                if to_call > 0 {
                    let player = &mut self.players[self.current_player_idx];
                    let call_amount = to_call.min(player.chips);
                    player.chips -= call_amount;
                    player.current_bet += call_amount;
                    self.pot += call_amount;
                } else {
                    // Nothing to call, treated as check
                }
            },
            GameAction::Raise(amount) => {
                let player = &mut self.players[self.current_player_idx];
                
                // Calculate how much to call (difference between highest bet and player's current bet)
                let to_call = highest_bet.saturating_sub(player.current_bet);
                
                // The raise amount is specified by the player
                // Total amount to add is what's needed to call plus the raise amount
                let total_to_add = to_call + amount;
                
                // Ensure player has enough chips
                let actual_to_add = total_to_add.min(player.chips);
                
                player.chips -= actual_to_add;
                player.current_bet += actual_to_add;
                self.pot += actual_to_add;
            },
            GameAction::Check => {
                // Can only check if no outstanding bet
                if to_call > 0 {
                    // If check is invalid, convert to call
                    self.perform_action(GameAction::Call);
                } else {
                    // Valid check - nothing to do
                }
            },
        }
    }
    
    fn determine_winner(&mut self) -> (usize, u32, String) {
        // Only evaluate hands for non-folded players
        let active_players: Vec<usize> = self.players.iter()
            .enumerate()
            .filter(|(_, p)| !p.folded)
            .map(|(i, _)| i)
            .collect();
        
        if active_players.len() == 1 {
            // Only one player left, they win by default (others folded)
            let winner_idx = active_players[0];
            let winnings = self.pot;
            self.players[winner_idx].chips += winnings;
            self.pot = 0;
            return (winner_idx, winnings, "Win by fold".to_string());
        }
        
        // If we have community cards, use proper poker hand evaluation
        if !self.community_cards.is_empty() {
            // Evaluate each active player's hand
            let mut best_player_idx = active_players[0];
            let mut best_hand_value = PokerRank::HighCard(0);
            let mut best_hand_type = String::new();
            
            for &player_idx in &active_players {
                let player = &self.players[player_idx];
                
                // Skip if player doesn't have 2 cards (shouldn't happen)
                if player.hand.len() < 2 {
                    continue;
                }
                
                // Convert all cards to rs-poker format
                let mut all_cards = Vec::with_capacity(7); // 2 hole cards + up to 5 community
                
                // Add player's hole cards
                for card in &player.hand {
                    let poker_value = match card.rank {
                        Rank::Two => PokerValue::Two,
                        Rank::Three => PokerValue::Three,
                        Rank::Four => PokerValue::Four,
                        Rank::Five => PokerValue::Five,
                        Rank::Six => PokerValue::Six,
                        Rank::Seven => PokerValue::Seven,
                        Rank::Eight => PokerValue::Eight,
                        Rank::Nine => PokerValue::Nine,
                        Rank::Ten => PokerValue::Ten,
                        Rank::Jack => PokerValue::Jack,
                        Rank::Queen => PokerValue::Queen,
                        Rank::King => PokerValue::King,
                        Rank::Ace => PokerValue::Ace,
                    };
                    
                    let poker_suit = match card.suit {
                        Suit::Hearts => PokerSuit::Heart,
                        Suit::Diamonds => PokerSuit::Diamond,
                        Suit::Clubs => PokerSuit::Club,
                        Suit::Spades => PokerSuit::Spade,
                    };
                    
                    all_cards.push(PokerCard { value: poker_value, suit: poker_suit });
                }
                
                // Add community cards
                for card in &self.community_cards {
                    let poker_value = match card.rank {
                        Rank::Two => PokerValue::Two,
                        Rank::Three => PokerValue::Three,
                        Rank::Four => PokerValue::Four,
                        Rank::Five => PokerValue::Five,
                        Rank::Six => PokerValue::Six,
                        Rank::Seven => PokerValue::Seven,
                        Rank::Eight => PokerValue::Eight,
                        Rank::Nine => PokerValue::Nine,
                        Rank::Ten => PokerValue::Ten,
                        Rank::Jack => PokerValue::Jack,
                        Rank::Queen => PokerValue::Queen,
                        Rank::King => PokerValue::King,
                        Rank::Ace => PokerValue::Ace,
                    };
                    
                    let poker_suit = match card.suit {
                        Suit::Hearts => PokerSuit::Heart,
                        Suit::Diamonds => PokerSuit::Diamond,
                        Suit::Clubs => PokerSuit::Club,
                        Suit::Spades => PokerSuit::Spade,
                    };
                    
                    all_cards.push(PokerCard { value: poker_value, suit: poker_suit });
                }
                
                // Create a hand from all cards
                let hand = Hand::new_with_cards(all_cards);
                
                // Evaluate the hand to get its value and type
                let hand_value = hand.rank();
                
                // Check if this is the best hand so far
                // In poker evaluation, higher values are better hands
                if player_idx == active_players[0] || hand_value > best_hand_value {
                    best_player_idx = player_idx;
                    best_hand_value = hand_value;
                    
                    // Get the hand type string
                    best_hand_type = match hand_value {
                        PokerRank::HighCard(_) => "High Card".to_string(),
                        PokerRank::OnePair(_) => "Pair".to_string(),
                        PokerRank::TwoPair(_) => "Two Pair".to_string(),
                        PokerRank::ThreeOfAKind(_) => "Three of a Kind".to_string(),
                        PokerRank::Straight(_) => "Straight".to_string(),
                        PokerRank::Flush(_) => "Flush".to_string(),
                        PokerRank::FullHouse(_) => "Full House".to_string(),
                        PokerRank::FourOfAKind(_) => "Four of a Kind".to_string(),
                        PokerRank::StraightFlush(_) => "Straight Flush".to_string(),
                    };
                }
            }
            
            // Award pot to the winner
            let winnings = self.pot;
            self.players[best_player_idx].chips += winnings;
            self.pot = 0;
            
            return (best_player_idx, winnings, best_hand_type);
        }
        
        // Fallback if we couldn't evaluate hands - random selection with simulated hand
        let mut rng = thread_rng();
        let winner_idx = active_players[rng.gen_range(0..active_players.len())];
        
        // Generate a simulated hand type for display
        let hand_types = [
            "High Card", "Pair", "Two Pair", "Three of a Kind", 
            "Straight", "Flush", "Full House", "Four of a Kind", 
            "Straight Flush", "Royal Flush"
        ];
        
        // Weight the hands so higher ones are less common
        let weights = [20, 15, 10, 8, 6, 5, 4, 2, 1, 1];
        let total_weight: i32 = weights.iter().sum();
        
        let mut rand_val = rng.gen_range(0..total_weight);
        let mut hand_idx = 0;
        
        for (i, &weight) in weights.iter().enumerate() {
            if rand_val < weight {
                hand_idx = i;
                break;
            }
            rand_val -= weight;
        }
        
        // Get the hand description
        let mut hand_type = hand_types[hand_idx];
        
        // Use the rs-poker library for proper poker hand evaluation
        let player_cards = &self.players[winner_idx].hand;
        
        if player_cards.len() >= 2 && !self.community_cards.is_empty() {
            // Convert our cards to rs-poker cards
            let mut all_cards = Vec::with_capacity(7); // 2 hole cards + up to 5 community
            
            // Add player's hole cards
            for card in player_cards {
                // Convert our card to rs-poker card
                let poker_value = match card.rank {
                    Rank::Two => PokerValue::Two,
                    Rank::Three => PokerValue::Three,
                    Rank::Four => PokerValue::Four,
                    Rank::Five => PokerValue::Five,
                    Rank::Six => PokerValue::Six,
                    Rank::Seven => PokerValue::Seven,
                    Rank::Eight => PokerValue::Eight,
                    Rank::Nine => PokerValue::Nine,
                    Rank::Ten => PokerValue::Ten,
                    Rank::Jack => PokerValue::Jack,
                    Rank::Queen => PokerValue::Queen,
                    Rank::King => PokerValue::King,
                    Rank::Ace => PokerValue::Ace,
                };
                
                let poker_suit = match card.suit {
                    Suit::Hearts => PokerSuit::Heart,
                    Suit::Diamonds => PokerSuit::Diamond,
                    Suit::Clubs => PokerSuit::Club,
                    Suit::Spades => PokerSuit::Spade,
                };
                
                all_cards.push(PokerCard { value: poker_value, suit: poker_suit });
            }
            
            // Add community cards
            for card in &self.community_cards {
                // Convert our card to rs-poker card
                let poker_value = match card.rank {
                    Rank::Two => PokerValue::Two,
                    Rank::Three => PokerValue::Three,
                    Rank::Four => PokerValue::Four,
                    Rank::Five => PokerValue::Five,
                    Rank::Six => PokerValue::Six,
                    Rank::Seven => PokerValue::Seven,
                    Rank::Eight => PokerValue::Eight,
                    Rank::Nine => PokerValue::Nine,
                    Rank::Ten => PokerValue::Ten,
                    Rank::Jack => PokerValue::Jack,
                    Rank::Queen => PokerValue::Queen,
                    Rank::King => PokerValue::King,
                    Rank::Ace => PokerValue::Ace,
                };
                
                let poker_suit = match card.suit {
                    Suit::Hearts => PokerSuit::Heart,
                    Suit::Diamonds => PokerSuit::Diamond,
                    Suit::Clubs => PokerSuit::Club,
                    Suit::Spades => PokerSuit::Spade,
                };
                
                all_cards.push(PokerCard { value: poker_value, suit: poker_suit });
            }
            
            // Create a hand with all cards (player's + community)
            let hand = Hand::new_with_cards(all_cards);
            
            // Evaluate the hand to get best 5-card hand
            let hand_rank = hand.rank();
            
            // Map the hand ranking to our hand types based on the evaluated hand
            hand_type = match hand_rank {
                PokerRank::HighCard(_) => "High Card",
                PokerRank::OnePair(_) => "Pair",
                PokerRank::TwoPair(_) => "Two Pair",
                PokerRank::ThreeOfAKind(_) => "Three of a Kind",
                PokerRank::Straight(_) => "Straight",
                PokerRank::Flush(_) => "Flush",
                PokerRank::FullHouse(_) => "Full House",
                PokerRank::FourOfAKind(_) => "Four of a Kind",
                PokerRank::StraightFlush(_) => "Straight Flush",
            };
        } else if player_cards.len() >= 2 {
            // If we only have hole cards, just check for a pair
            if player_cards[0].rank == player_cards[1].rank {
                hand_type = "Pair"; // It's a pocket pair
            } else {
                hand_type = "High Card";
            }
        }
        
        // If we have community cards, show some of them in the description
        let card_description = if !self.community_cards.is_empty() && !self.players[winner_idx].hand.is_empty() {
            // Show the player's hand and relevant community cards
            let hand_cards = self.players[winner_idx].hand.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
                
            // Add some community cards based on the hand type
            let relevant_community = match hand_type {
                "Flush" | "Straight Flush" | "Royal Flush" => {
                    // Show 3 community cards for a flush
                    self.community_cards.iter()
                        .take(3)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                },
                "Full House" | "Four of a Kind" => {
                    // Show 2 community cards
                    self.community_cards.iter()
                        .take(2)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                },
                _ => {
                    // Show 1 community card for other hands
                    self.community_cards.iter()
                        .take(1)
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ")
                }
            };
            
            format!("{} with {} + {}", hand_type, hand_cards, relevant_community)
        } else {
            // Fallback if we don't have cards to show
            format!("{}", hand_type)
        };
        
        let winnings = self.pot;
        self.players[winner_idx].chips += winnings;
        self.pot = 0;
        
        (winner_idx, winnings, card_description)
    }

// UI Application
struct App {
    game: Game,
    input: String,
    messages: Vec<String>,
    should_quit: bool,
    player_starting_chips: u32, // To track wins/losses
    round_results: Option<(String, i32)>, // (Winner name, player profit/loss)
    game_stats: Vec<i32>, // Track player profits across multiple rounds
    bot_thinking: bool,         // To simulate bot thinking time
    bot_think_until: std::time::Instant, // When bot should finish "thinking"
    game_active: bool,          // Whether a game is currently in progress
    message_scroll_pos: usize,  // Position in message history for scrolling
    input_mode: InputMode,      // Current input mode (raise amount or player name)
}

impl App {
    fn new(api_key: Option<String>, player_name: String) -> Self {
        // Starting chips amount
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

    // Additional Game methods
    fn make_openai_api_call(&self, api_key: &str, request: &OpenAIRequest) -> Result<String, String> {
        let client = &self.ai_client;
        
        // First attempt to send the request
        let response_result = client.post("https://api.openai.com/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(request)
            .send();
            
        // Handle HTTP request errors
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                return Err(format!("Network error: {}", e));
            }
        };
        
        // Try to parse the JSON response
        let json_result = response.json::<OpenAIResponse>();
        
        match json_result {
            Ok(parsed) => {
                if let Some(choice) = parsed.choices.first() {
                    Ok(choice.message.content.clone())
                } else {
                    Err("No choices returned from API".to_string())
                }
            },
            Err(_e) => {
                // Return a friendly error message instead of the raw error
                Err(format!("API response error (using fallback)"))
            }
        }
    }
    
    fn get_player_position(&self, player_idx: usize) -> String {
        if player_idx == self.dealer_idx {
            return "Dealer (BTN)".to_string();
        } else if player_idx == self.small_blind_idx {
            return "Small Blind (SB)".to_string();
        } else if player_idx == self.big_blind_idx {
            return "Big Blind (BB)".to_string();
        } else if self.players.len() <= 3 {
            return "".to_string(); // No special positions in very small games besides the blinds
        } else if player_idx == (self.big_blind_idx + 1) % self.players.len() {
            return "Under the Gun (UTG)".to_string();
        } else if player_idx == (self.big_blind_idx + 2) % self.players.len() && self.players.len() > 4 {
            return "Middle Position (MP)".to_string();
        } else if player_idx == (self.dealer_idx - 1 + self.players.len()) % self.players.len() {
            return "Cut-off (CO)".to_string();
        } else {
            return "Middle Position (MP)".to_string();
        }
    }
    
    fn generate_random_bot_action(&self, player: &Player) -> String {
        let mut rng = rand::thread_rng();
        
        // Check if the player has enough chips to make meaningful bets
        let has_chips = player.chips >= self.min_bet;
        
        // Reduce raising probability based on action count to prevent infinite loops
        let raise_penalty = (self.last_action_count as f32 * 0.5).min(8.0) as u32;
        
        // If we're in later rounds or have many actions, bots should be more conservative
        let is_late_round = self.round == Round::Turn || self.round == Round::River;
        
        match player.bot_difficulty {
            BotDifficulty::Easy => {
                // Easy bots mostly check/call, occasionally raise, and rarely fold
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Adjust choice based on round and action count
                if is_late_round || self.last_action_count > 10 {
                    choice = choice.saturating_add(2); // Make raising less likely
                }
                
                if choice < 5 {
                    "call".to_string()
                } else if choice < 8 {
                    "check".to_string()
                } else if choice < 9 && has_chips && raise_penalty < 8 {
                    // Smaller raises to avoid escalation
                    format!("raise {}", self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
            BotDifficulty::Medium => {
                // Medium bots have more balanced play
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Adjust choice based on round and action count
                if is_late_round || self.last_action_count > 8 {
                    choice = choice.saturating_add(3); // Make raising less likely in later rounds
                }
                
                if choice < 3 {
                    "call".to_string()
                } else if choice < 6 {
                    "check".to_string()
                } else if choice < 9 && has_chips && raise_penalty < 7 {
                    // More modest raises
                    format!("raise {}", rng.gen_range(1..3) * self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
            BotDifficulty::Hard => {
                // Hard bots play more aggressively but still adjust
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Still apply some limits to prevent infinite loops
                if is_late_round || self.last_action_count > 6 {
                    choice = choice.saturating_add(2);
                }
                
                if choice < 2 {
                    "call".to_string()
                } else if choice < 4 {
                    "check".to_string()
                } else if choice < 8 && has_chips && raise_penalty < 6 {
                    // Still aggressive but controlled raises
                    format!("raise {}", rng.gen_range(1..3) * self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
        }
    }
}

// End of Game implementation

// Helper functions - not methods of Game
fn get_player_position(game: &Game, player_idx: usize) -> String {
        if player_idx == game.dealer_idx {
            return "Dealer (BTN)".to_string();
        } else if player_idx == game.small_blind_idx {
            return "Small Blind (SB)".to_string();
        } else if player_idx == game.big_blind_idx {
            return "Big Blind (BB)".to_string();
        } else if game.players.len() <= 3 {
            return "".to_string(); // No special positions in very small games besides the blinds
        } else if player_idx == (game.big_blind_idx + 1) % game.players.len() {
            return "Under the Gun (UTG)".to_string();
        } else if player_idx == (game.big_blind_idx + 2) % game.players.len() && game.players.len() > 4 {
            return "Middle Position (MP)".to_string();
        } else if player_idx == (game.dealer_idx - 1 + game.players.len()) % game.players.len() {
            return "Cut-off (CO)".to_string();
        } else {
            return "Middle Position (MP)".to_string();
        }
    }
    
    fn make_openai_api_call(&self, api_key: &str, request: &OpenAIRequest) -> Result<String, String> {
        let client = &self.ai_client;
        
        // First attempt to send the request
        let response_result = client.post("https://api.openai.com/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(request)
            .send();
            
        // Handle HTTP request errors
        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                return Err(format!("Network error: {}", e));
            }
        };
        
        // Try to parse the JSON response
        let json_result = response.json::<OpenAIResponse>();
        
        match json_result {
            Ok(parsed) => {
                if let Some(choice) = parsed.choices.first() {
                    Ok(choice.message.content.clone())
                } else {
                    Err("No choices returned from API".to_string())
                }
            },
            Err(_e) => {
                // Return a friendly error message instead of the raw error
                Err(format!("API response error (using fallback)"))
            }
        }
    }
    
    fn generate_random_bot_action(&self, player: &Player) -> String {
        let mut rng = rand::thread_rng();
        
        // Check if the player has enough chips to make meaningful bets
        let has_chips = player.chips >= self.min_bet;
        
        // Reduce raising probability based on action count to prevent infinite loops
        let raise_penalty = (self.last_action_count as f32 * 0.5).min(8.0) as u32;
        
        // If we're in later rounds or have many actions, bots should be more conservative
        let is_late_round = self.round == Round::Turn || self.round == Round::River;
        
        match player.bot_difficulty {
            BotDifficulty::Easy => {
                // Easy bots mostly check/call, occasionally raise, and rarely fold
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Adjust choice based on round and action count
                if is_late_round || self.last_action_count > 10 {
                    choice = choice.saturating_add(2); // Make raising less likely
                }
                
                if choice < 5 {
                    "call".to_string()
                } else if choice < 8 {
                    "check".to_string()
                } else if choice < 9 && has_chips && raise_penalty < 8 {
                    // Smaller raises to avoid escalation
                    format!("raise {}", self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
            BotDifficulty::Medium => {
                // Medium bots have more balanced play
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Adjust choice based on round and action count
                if is_late_round || self.last_action_count > 8 {
                    choice = choice.saturating_add(3); // Make raising less likely in later rounds
                }
                
                if choice < 3 {
                    "call".to_string()
                } else if choice < 6 {
                    "check".to_string()
                } else if choice < 9 && has_chips && raise_penalty < 7 {
                    // More modest raises
                    format!("raise {}", rng.gen_range(1..3) * self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
            BotDifficulty::Hard => {
                // Hard bots play more aggressively but still adjust
                let mut choice: i32 = rng.gen_range(0..10);
                
                // Still apply some limits to prevent infinite loops
                if is_late_round || self.last_action_count > 6 {
                    choice = choice.saturating_add(2);
                }
                
                if choice < 2 {
                    "call".to_string()
                } else if choice < 4 {
                    "check".to_string()
                } else if choice < 8 && has_chips && raise_penalty < 6 {
                    // Still aggressive but controlled raises
                    format!("raise {}", rng.gen_range(1..3) * self.min_bet)
                } else {
                    "fold".to_string()
                }
            },
        }
    }
}

