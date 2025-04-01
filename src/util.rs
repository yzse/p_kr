use crate::game::Game;

// Helper function to get player position name
pub fn get_player_position(game: &Game, player_idx: usize) -> String {
    if player_idx == game.dealer_idx {
        return "Button (BTN)".to_string();
    } else if player_idx == game.small_blind_idx {
        return "Small Blind (SB)".to_string();
    } else if player_idx == game.big_blind_idx {
        return "Big Blind (BB)".to_string();
    } else if game.players.len() <= 3 {
        return "".to_string(); // No special positions in very small games besides the blinds
    } 
    
    // Calculate position based on distance from BB
    let num_players = game.players.len();
    let distance_from_bb = (player_idx + num_players - game.big_blind_idx) % num_players;
    
    match distance_from_bb {
        1 => "Under The Gun (UTG)".to_string(),
        2 => "UTG+1".to_string(),
        3 => "UTG+2".to_string(),
        4 => {
            if num_players >= 7 {
                "Middle Position (MP)".to_string()
            } else {
                "Hijack (HJ)".to_string()
            }
        },
        5 => {
            if num_players >= 8 {
                "Middle Position +1 (MP+1)".to_string() 
            } else {
                "Hijack (HJ)".to_string()
            }
        },
        6 => "Hijack (HJ)".to_string(),
        7 => "Cut-off (CO)".to_string(),
        // Player right before dealer would be CO
        _ => {
            if player_idx == (game.dealer_idx + num_players - 1) % num_players {
                "Cut-off (CO)".to_string()
            } else if player_idx == (game.dealer_idx + num_players - 2) % num_players {
                "Hijack (HJ)".to_string()
            } else {
                "Middle Position (MP)".to_string()
            }
        }
    }
}