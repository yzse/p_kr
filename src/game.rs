use rand::prelude::*;
use rand::Rng;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use rs_poker::core::{Card as PokerCard, Suit as PokerSuit, Value as PokerValue, Hand, Rank as PokerRank, Rankable};

// Card representation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rank {
    Two, Three, Four, Five, Six, Seven, Eight, Nine, Ten,
    Jack, Queen, King, Ace,
}

impl Rank {
    pub fn to_string(&self) -> String {
        match self {
            Rank::Two => "2".to_string(),
            Rank::Three => "3".to_string(),
            Rank::Four => "4".to_string(),
            Rank::Five => "5".to_string(),
            Rank::Six => "6".to_string(),
            Rank::Seven => "7".to_string(),
            Rank::Eight => "8".to_string(),
            Rank::Nine => "9".to_string(),
            Rank::Ten => "10".to_string(),
            Rank::Jack => "J".to_string(),
            Rank::Queen => "Q".to_string(),
            Rank::King => "K".to_string(),
            Rank::Ace => "A".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suit {
    Hearts, Diamonds, Clubs, Spades,
}

impl Suit {
    pub fn to_string(&self) -> String {
        match self {
            Suit::Hearts => "Hearts".to_string(),
            Suit::Diamonds => "Diamonds".to_string(),
            Suit::Clubs => "Clubs".to_string(),
            Suit::Spades => "Spades".to_string(),
        }
    }
}

impl Card {
    pub fn to_string(&self) -> String {
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
pub enum HandRank {
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
pub struct Player {
    pub name: String,
    pub hand: Vec<Card>,
    pub chips: u32,
    pub current_bet: u32,
    pub folded: bool,
    pub is_bot: bool,
    pub bot_difficulty: BotDifficulty,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum BotDifficulty {
    Easy,
    Medium,
    Hard,
}

// Game state
pub struct Game {
    pub players: Vec<Player>,
    pub deck: Vec<Card>,
    pub community_cards: Vec<Card>,
    pub pot: u32,
    pub current_player_idx: usize,
    pub min_bet: u32,
    pub round: Round,
    pub ai_client: Client,
    pub api_key: Option<String>,
    pub dealer_idx: usize,
    pub small_blind_idx: usize,
    pub big_blind_idx: usize,
    pub last_action_count: usize, // Track number of consecutive bot actions
    pub bb_has_acted_preflop: bool, // Track if BB has acted in pre-flop
    pub players_acted_this_round: Vec<usize>, // Track which players have already acted in current round
    pub last_aggressor: Option<usize>, // Track the last player who bet or raised
    pub round_action_complete: bool, // Flag for whether a round of betting is complete
    pub player_contributions_this_round: Vec<u32>, // Track how much each player has contributed in the current round
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Round {
    PreFlop,
    Flop,
    Turn,
    River,
    Showdown,
}

#[derive(Serialize, Deserialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct OpenAIResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: Message,
}

#[derive(Clone, Debug)]
pub enum GameAction {
    Fold,
    Call,
    Raise(u32),
    Check,
}

// Game implementation
impl Game {
    pub fn new(num_human_players: usize, num_bot_players: usize, bot_difficulty: BotDifficulty, starting_chips: u32, api_key: Option<String>, player_name: String) -> Self {
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
        
        // Create player_contributions_this_round with the same length as players, initialized to 0
        let player_contributions_this_round = vec![0; players.len()];
        
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
            bb_has_acted_preflop: false,
            players_acted_this_round: Vec::new(),
            last_aggressor: None,
            round_action_complete: false,
            player_contributions_this_round,
        }
    }
    
    pub fn create_deck() -> Vec<Card> {
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
    
    pub fn shuffle_deck(&mut self) {
        let mut rng = thread_rng();
        self.deck.shuffle(&mut rng);
    }
    
    pub fn deal_cards(&mut self) {
        // Reset action counter
        self.last_action_count = 0;
        
        // Reset BB action tracking
        self.bb_has_acted_preflop = false;
        
        // Reset round action tracking
        self.players_acted_this_round = Vec::new();
        self.last_aggressor = None;
        self.round_action_complete = false;
        
        // Reset player contributions for the new round
        self.player_contributions_this_round = vec![0; self.players.len()];
        
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
        for (idx, player) in self.players.iter_mut().enumerate() {
            player.chips = player.chips.saturating_sub(ante);
            self.pot += ante;
            // Track the ante contribution
            self.player_contributions_this_round[idx] += ante;
        }
        
        if self.players.len() >= 2 {
            // Small blind (minimum 5)
            let small_blind = self.min_bet / 2;
            self.players[self.small_blind_idx].chips = self.players[self.small_blind_idx].chips.saturating_sub(small_blind);
            self.players[self.small_blind_idx].current_bet = small_blind;
            self.pot += small_blind;
            // Track the small blind contribution
            self.player_contributions_this_round[self.small_blind_idx] += small_blind;
            
            // Big blind (minimum 10)
            let big_blind = self.min_bet;
            self.players[self.big_blind_idx].chips = self.players[self.big_blind_idx].chips.saturating_sub(big_blind);
            self.players[self.big_blind_idx].current_bet = big_blind;
            self.pot += big_blind;
            // Track the big blind contribution
            self.player_contributions_this_round[self.big_blind_idx] += big_blind;
            
            // Start with player after big blind (UTG position)
            self.current_player_idx = (self.big_blind_idx + 1) % self.players.len();
        }
    }
    
    pub fn deal_community_cards(&mut self) {
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
    
    pub fn next_round(&mut self) {
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
        
        // Debug info - we'll use this for development only
        // We'll keep it commented out to avoid UI interference
        /*
        let round_name = match self.round {
            Round::PreFlop => "Pre-Flop",
            Round::Flop => "Flop",
            Round::Turn => "Turn",
            Round::River => "River",
            Round::Showdown => "Showdown",
        };
        println!("Round transition: {} -> {}", 
            match previous_round {
                Round::PreFlop => "Pre-Flop",
                Round::Flop => "Flop", 
                Round::Turn => "Turn",
                Round::River => "River",
                Round::Showdown => "Showdown",
            }, 
            round_name
        );
        */
        
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
            Round::Showdown => {
                // Ensure we have all 5 community cards
                if self.community_cards.len() < 5 {
                    // Missing cards
                    let missing = 5 - self.community_cards.len();
                    for _ in 0..missing {
                        if let Some(card) = self.deck.pop() {
                            self.community_cards.push(card);
                        }
                    }
                }
                // No additional cards dealt in Showdown
            },
            _ => {}
        }
        
        // Reset action counter for new round - critical for proper round management
        self.last_action_count = 0; // Ensure counter is reset for new round
        
        // Reset action tracking for the new round
        self.players_acted_this_round.clear();
        self.last_aggressor = None;
        self.round_action_complete = false;
        
        // Reset player contributions for the new round
        self.player_contributions_this_round = vec![0; self.players.len()];
        
        // Set a minimum number of actions required based on active players
        // This ensures all players get a chance to act in each round
        // (This is used for reference but now handled by players_acted_this_round)
        
        // Reset to first active player after small blind (not after dealer)
        // For Showdown, we don't need to set the player index as we'll determine winner immediately
        if self.round != Round::Showdown {
            // Start with the player after small blind in post-flop rounds
            self.current_player_idx = self.find_next_active_player((self.small_blind_idx) % self.players.len());
        }
    }
    
    pub fn find_next_active_player(&self, current_idx: usize) -> usize {
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
    
    pub fn next_player(&mut self) -> bool {
        // STEP 1: Check if the round is over by counting active players
        let active_players = self.players.iter().filter(|p| !p.folded && p.chips > 0).count();
        
        // If only one player remains, the round (and game) is over
        if active_players <= 1 {
            // Special case: only one player left in the hand
            if self.round == Round::Showdown {
                // End the hand and determine winner
                return false;
            } else {
                // Skip to showdown since there's only one player left
                self.round = Round::Showdown;
                return true;
            }
        }
        
        // STEP 2: Check if all betting actions are complete for the current round
        let highest_bet = self.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        
        // Check if all players have either matched the highest bet, folded, or are all-in
        let bets_matched = self.players.iter()
            .filter(|p| !p.folded)  // Only consider active players
            .all(|p| p.current_bet == highest_bet || p.chips == 0);  // All have matched or are all-in
            
        // Force advancement after too many actions (safeguard)
        let force_advancement = self.last_action_count >= active_players * 3;
        
        // Special check for PreFlop - ensure big blind has acted
        let bb_rule_satisfied = if self.round == Round::PreFlop {
            // Only consider the round complete if BB has acted
            self.bb_has_acted_preflop || self.players[self.big_blind_idx].folded
        } else {
            true
        };
        
        // Track the last player who raised (last aggressor)
        // If there's a last aggressor, we need to make sure everyone has acted after them
        let all_acted_after_aggressor = if let Some(aggressor_idx) = self.last_aggressor {
            // Check if everyone has acted since the last aggressor
            let mut idx = self.find_next_active_player(aggressor_idx);
            let mut all_acted = true;
            
            while idx != aggressor_idx {
                // If this player hasn't acted since the last raise AND they're not folded/all-in
                if !self.players_acted_this_round.contains(&idx) && 
                   !self.players[idx].folded && 
                   self.players[idx].chips > 0 {
                    all_acted = false;
                    break;
                }
                idx = self.find_next_active_player(idx);
            }
            
            all_acted
        } else {
            // If no aggressor, check if all active players have acted at least once
            let active_player_indices: Vec<usize> = self.players.iter()
                .enumerate()
                .filter(|(_, p)| !p.folded && p.chips > 0)
                .map(|(idx, _)| idx)
                .collect();
                
            active_player_indices.iter().all(|idx| self.players_acted_this_round.contains(idx))
        };
        
        // Determine if the round is complete
        let round_complete = (bets_matched && bb_rule_satisfied && all_acted_after_aggressor) || force_advancement;
        
        if round_complete {
            if self.round == Round::Showdown {
                // Game is over - determine winner and distribute pot
                return false;
            } else {
                // Move to the next round
                self.next_round();
                return true;
            }
        }
        
        // STEP 3: Move to the next player who still needs to act
        
        // Track if BB has acted in PreFlop
        if self.round == Round::PreFlop && self.current_player_idx == self.big_blind_idx {
            self.bb_has_acted_preflop = true;
        }
        
        // Find the next active player
        if let Some(aggressor_idx) = self.last_aggressor {
            // If there was a raise, start from after the aggressor to ensure everyone responds
            let start_idx = self.find_next_active_player(aggressor_idx);
            self.current_player_idx = start_idx;
            
            // Find the first player who still needs to act (after aggressor)
            while self.players[self.current_player_idx].folded || 
                  self.players[self.current_player_idx].chips == 0 ||
                  (self.players_acted_this_round.contains(&self.current_player_idx) && 
                   self.players[self.current_player_idx].current_bet == highest_bet) {
                
                self.current_player_idx = self.find_next_active_player(self.current_player_idx);
                
                // If we've looped back to the aggressor, everyone has acted
                if self.current_player_idx == aggressor_idx {
                    // Round complete since we've gone full circle
                    if self.round == Round::Showdown {
                        return false; // End the hand
                    } else {
                        self.next_round();
                        return true;
                    }
                }
            }
        } else {
            // If no aggressor (everyone checked so far), just move to next player
            self.current_player_idx = self.find_next_active_player(self.current_player_idx);
        }
        
        // Game continues with the next player
        true
    }
    
    pub fn perform_action(&mut self, action: GameAction) -> (GameAction, Option<u32>) {
        // Get the current player index
        let current_player_idx = self.current_player_idx;
        
        // Calculate highest bet among players
        let highest_bet = self.players.iter().map(|p| p.current_bet).max().unwrap_or(0);
        
        // Get player's current bet before modification
        let player_current_bet = self.players[current_player_idx].current_bet;
        
        // Get player's contribution this round before modification
        let player_contribution_before = self.player_contributions_this_round[current_player_idx];
        
        // Determine if this is the first bet in this round
        let is_first_bet_in_round = highest_bet == 0;
        
        // Increment the action counter when a player acts
        // This is critical for proper round management
        self.last_action_count += 1;
        
        // Track that this player has acted in this round
        if !self.players_acted_this_round.contains(&current_player_idx) {
            self.players_acted_this_round.push(current_player_idx);
        }
        
        // Store initial chip stack and pot for validation
        let initial_chips = self.players[current_player_idx].chips;
        let initial_pot = self.pot;
        
        // The action we'll actually perform (may be different from requested)
        // The second value is the total bet after action (for display purposes)
        let actual_action = match action {
            GameAction::Fold => {
                // Player folds their hand
                self.players[current_player_idx].folded = true;
                // No chips are contributed when folding
                (GameAction::Fold, None)
            },
            GameAction::Call => {
                // Check if there's actually a bet to call
                if highest_bet <= player_current_bet {
                    // No bet to call - convert to check
                    (GameAction::Check, Some(player_current_bet))
                } else {
                    // Player calls the highest bet
                    let call_amount = highest_bet.saturating_sub(player_current_bet);
                    
                    // Don't allow calling more than player has
                    let actual_call = call_amount.min(self.players[current_player_idx].chips);
                    
                    // Update game state
                    self.players[current_player_idx].chips -= actual_call;
                    self.players[current_player_idx].current_bet += actual_call;
                    self.pot += actual_call;
                    
                    // Update player's contribution for this round
                    self.player_contributions_this_round[current_player_idx] += actual_call;
                    
                    // Return the action and the changed values for logging
                    (GameAction::Call, Some(self.players[current_player_idx].current_bet))
                }
            },
            GameAction::Raise(amount) => {
                // If this is the first bet in the round, it should be called a "bet" not a "raise"
                if is_first_bet_in_round {
                    // This is a bet, not a raise
                    // Don't allow betting more than player has
                    let actual_bet = amount.min(self.players[current_player_idx].chips);
                    
                    if actual_bet < self.min_bet {
                        // Not enough for minimum bet - convert to check
                        (GameAction::Check, Some(0))
                    } else {
                        // Perform the bet
                        self.players[current_player_idx].chips -= actual_bet;
                        self.players[current_player_idx].current_bet = actual_bet; // Not += because it's a new bet
                        self.pot += actual_bet;
                        
                        // Update player's contribution for this round
                        self.player_contributions_this_round[current_player_idx] += actual_bet;
                        
                        // Set this player as the last aggressor
                        self.last_aggressor = Some(current_player_idx);
                        
                        // Reset acted list to only include this player
                        self.players_acted_this_round.clear();
                        self.players_acted_this_round.push(current_player_idx);
                        
                        (GameAction::Raise(actual_bet), Some(actual_bet)) // We'll convert this to "bet" in display
                    }
                } else {
                    // This is a raise (there was a previous bet)
                    // Raising requires at least the minimum bet above current highest
                    let _min_raise = (highest_bet + self.min_bet).saturating_sub(player_current_bet); // Used in comments for clarity
                    
                    // Calculate final bet amount after raise
                    let target_bet = player_current_bet + amount;
                    
                    // Check if the raise amount is sufficient
                    if target_bet < highest_bet + self.min_bet {
                        // Raise amount too small
                        if highest_bet > player_current_bet {
                            // There's a bet to call
                            let call_amount = highest_bet.saturating_sub(player_current_bet);
                            let actual_call = call_amount.min(self.players[current_player_idx].chips);
                            
                            self.players[current_player_idx].chips -= actual_call;
                            self.players[current_player_idx].current_bet += actual_call;
                            self.pot += actual_call;
                            
                            // Update player's contribution for this round
                            self.player_contributions_this_round[current_player_idx] += actual_call;
                            
                            (GameAction::Call, Some(self.players[current_player_idx].current_bet))
                        } else {
                            // No bet to call - convert to check
                            (GameAction::Check, Some(player_current_bet))
                        }
                    } else {
                        // Valid raise amount
                        // Calculate the actual amount to add to player's current bet
                        let raise_amount = target_bet.saturating_sub(player_current_bet);
                        
                        // Don't allow raising more than player has
                        let actual_raise = raise_amount.min(self.players[current_player_idx].chips);
                        let final_bet = player_current_bet + actual_raise;
                        
                        self.players[current_player_idx].chips -= actual_raise;
                        self.players[current_player_idx].current_bet += actual_raise;
                        self.pot += actual_raise;
                        
                        // Update player's contribution for this round
                        self.player_contributions_this_round[current_player_idx] += actual_raise;
                        
                        // Set this player as the last aggressor and reset who has acted
                        self.last_aggressor = Some(current_player_idx);
                        
                        // Reset acted list to only include this player
                        self.players_acted_this_round.clear();
                        self.players_acted_this_round.push(current_player_idx);
                        
                        (GameAction::Raise(actual_raise), Some(final_bet))
                    }
                }
            },
            GameAction::Check => {
                // Check is only valid if no one has bet yet or player has matched the highest bet
                if highest_bet > player_current_bet {
                    // Invalid check - convert to call
                    let call_amount = highest_bet.saturating_sub(player_current_bet);
                    
                    // Don't allow calling more than player has
                    let actual_call = call_amount.min(self.players[current_player_idx].chips);
                    
                    self.players[current_player_idx].chips -= actual_call;
                    self.players[current_player_idx].current_bet += actual_call;
                    self.pot += actual_call;
                    
                    // Update player's contribution for this round
                    self.player_contributions_this_round[current_player_idx] += actual_call;
                    
                    (GameAction::Call, Some(self.players[current_player_idx].current_bet))
                } else {
                    // Valid check - no chips are contributed
                    (GameAction::Check, Some(player_current_bet))
                }
            }
        };
        
        // Validation step - confirm pot increase matches player contribution
        let _player_contribution = self.player_contributions_this_round[current_player_idx] - player_contribution_before;
        let pot_increase = self.pot - initial_pot;
        let chip_decrease = initial_chips - self.players[current_player_idx].chips;
        
        // Assert that pot increase matches player's chip decrease
        if pot_increase != chip_decrease {
            println!("WARNING: Pot increase ({}) does not match player chip decrease ({})", 
                     pot_increase, chip_decrease);
        }
        
        // Return the actual action performed
        actual_action
    }
    
    pub fn determine_winner(&mut self) -> (usize, u32, String) {
        // Get active (non-folded) players
        let active_players: Vec<usize> = self.players.iter()
            .enumerate()
            .filter(|(_, player)| !player.folded)
            .map(|(idx, _)| idx)
            .collect();
            
        // If only one player remains, they win
        if active_players.len() == 1 {
            let winner_idx = active_players[0];
            let winnings = self.pot;
            self.players[winner_idx].chips += winnings;
            
            // Define a simple hand type for display
            let hand_type = if self.players[winner_idx].hand.is_empty() {
                "by default (others folded)".to_string()
            } else if self.community_cards.is_empty() {
                "with their hole cards".to_string()
            } else {
                "by being the last player standing".to_string()
            };
            
            self.pot = 0;
            return (winner_idx, winnings, hand_type);
        }
        
        // If more than one player, determine best hand
        let mut best_rank_value = 0;
        let mut best_actual_hand = None;
        let mut winner_idx = active_players[0]; // Default to first active player
        let mut winner_hand_type = "High Card".to_string();
        
        // Use poker hand evaluator to find winner
        for &player_idx in &active_players {
            // We combine player's hole cards with community cards
            let player_cards = &self.players[player_idx].hand;
            
            // Try to convert our cards to poker-rs format
            if !player_cards.is_empty() && !self.community_cards.is_empty() {
                let mut all_cards = Vec::new();
                
                // Process player cards
                for card in player_cards {
                    // Convert our rank to poker-rs Value
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
                    
                    // Convert our suit to poker-rs Suit
                    let poker_suit = match card.suit {
                        Suit::Hearts => PokerSuit::Heart,
                        Suit::Diamonds => PokerSuit::Diamond,
                        Suit::Clubs => PokerSuit::Club,
                        Suit::Spades => PokerSuit::Spade,
                    };
                    
                    all_cards.push(PokerCard { value: poker_value, suit: poker_suit });
                }
                
                // Process community cards
                for card in &self.community_cards {
                    // Convert our rank to poker-rs Value
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
                    
                    // Convert our suit to poker-rs Suit
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
                
                // Get numerical rank value for comparison
                let rank_value = match hand_rank {
                    PokerRank::HighCard(_) => 0,
                    PokerRank::OnePair(_) => 1,
                    PokerRank::TwoPair(_) => 2,
                    PokerRank::ThreeOfAKind(_) => 3,
                    PokerRank::Straight(_) => 4,
                    PokerRank::Flush(_) => 5,
                    PokerRank::FullHouse(_) => 6,
                    PokerRank::FourOfAKind(_) => 7,
                    PokerRank::StraightFlush(_) => 8,
                };

                // If this player has a better hand or this is the first player we're checking
                if rank_value > best_rank_value || best_actual_hand.is_none() {
                    // Update best rank and winner
                    best_rank_value = rank_value;
                    best_actual_hand = Some(hand_rank.clone());
                    winner_idx = player_idx;
                    
                    // Update the hand type string based on the rank
                    winner_hand_type = match hand_rank {
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
                // In case of a tie in hand rank category, we need to compare the actual hands
                // rs_poker's Rankable trait handles this by implementing PartialOrd
                else if rank_value == best_rank_value && best_actual_hand.is_some() {
                    if hand_rank > *best_actual_hand.as_ref().unwrap() {
                        best_actual_hand = Some(hand_rank.clone());
                        winner_idx = player_idx;
                        
                        // Update the hand type string based on the rank
                        winner_hand_type = match hand_rank {
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
            } else if player_cards.len() >= 2 {
                // If we only have hole cards (no community cards), just check for a pair
                if player_cards[0].rank == player_cards[1].rank {
                    // Only update if the current best hand is worse than a pair
                    if best_rank_value < 1 {
                        best_rank_value = 1; // Pair
                        winner_idx = player_idx;
                        winner_hand_type = "Pair".to_string();
                    }
                } else {
                    // High card - only update if we haven't found anything better yet
                    if best_rank_value == 0 && best_actual_hand.is_none() {
                        winner_idx = player_idx;
                        winner_hand_type = "High Card".to_string();
                    }
                }
            }
        }
        
        // Create a descriptive string for the winning hand
        let card_description = if !self.community_cards.is_empty() && !self.players[winner_idx].hand.is_empty() {
            // Get the winner's hole cards
            let hole_cards = self.players[winner_idx].hand.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            
            match winner_hand_type.as_str() {
                "High Card" => format!("High Card with {}", hole_cards),
                "Pair" => {
                    // For a pair, show what the pair is if possible
                    let pair_in_hole = self.players[winner_idx].hand[0].rank == self.players[winner_idx].hand[1].rank;
                    if pair_in_hole {
                        format!("Pair of {}s", self.players[winner_idx].hand[0].rank.to_string())
                    } else {
                        // The pair includes one card from the community cards
                        format!("Pair with {}", hole_cards)
                    }
                },
                "Two Pair" => format!("Two Pair with {}", hole_cards),
                "Three of a Kind" => format!("Three of a Kind with {}", hole_cards),
                "Straight" => format!("Straight with {}", hole_cards),
                "Flush" => {
                    // For a flush, indicate the suit if all hole cards are the same suit
                    let same_suit = self.players[winner_idx].hand.len() == 2 &&
                        self.players[winner_idx].hand[0].suit == self.players[winner_idx].hand[1].suit;
                    
                    if same_suit {
                        format!("Flush ({}) with {}", self.players[winner_idx].hand[0].suit.to_string(), hole_cards)
                    } else {
                        format!("Flush with {}", hole_cards)
                    }
                },
                "Full House" => format!("Full House with {}", hole_cards),
                "Four of a Kind" => format!("Four of a Kind with {}", hole_cards),
                "Straight Flush" => format!("Straight Flush with {}", hole_cards),
                _ => format!("{} with {}", winner_hand_type, hole_cards),
            }
        } else {
            // Fallback if we don't have cards to show
            format!("{}", winner_hand_type)
        };
        
        let winnings = self.pot;
        self.players[winner_idx].chips += winnings;
        self.pot = 0;
        
        (winner_idx, winnings, card_description)
    }
    
    pub fn get_bot_action(&self, bot_player: &Player) -> Result<GameAction, String> {
        // Generate bot actions based on difficulty
        let action_str = self.generate_random_bot_action(bot_player);
        
        // Parse the action string
        if action_str.starts_with("fold") {
            Ok(GameAction::Fold)
        } else if action_str.starts_with("call") {
            Ok(GameAction::Call)
        } else if action_str.starts_with("check") {
            Ok(GameAction::Check)
        } else if action_str.starts_with("raise") {
            // Extract the raise amount
            let parts: Vec<&str> = action_str.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(amount) = parts[1].parse::<u32>() {
                    Ok(GameAction::Raise(amount))
                } else {
                    // Default raise amount
                    Ok(GameAction::Raise(self.min_bet))
                }
            } else {
                // Default raise amount
                Ok(GameAction::Raise(self.min_bet))
            }
        } else {
            // Default to checking
            Ok(GameAction::Check)
        }
    }
    
    pub fn make_openai_api_call(&self, api_key: &str, request: &OpenAIRequest) -> Result<String, String> {
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
    
    pub fn generate_random_bot_action(&self, player: &Player) -> String {
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