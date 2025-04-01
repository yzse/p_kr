mod game;
mod app;
mod util;
mod ui;

use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use tui::{
    backend::CrosstermBackend,
    Terminal,
};

use app::App;

fn main() -> Result<(), io::Error> {
    let api_key = std::env::var("OPENAI_API_KEY").ok();
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create app state
    let mut app = App::new(api_key, "Player 1".to_string());
    
    // Main game loop
    loop {
        // Handle bot actions if needed
        process_bot_actions(&mut app);
        
        // Draw the UI
        terminal.draw(|f| {
            ui::render_ui(f, &mut app);
        })?;
        
        // Handle events with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Process keys
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

// Process bot actions - extracted from the main loop to make it more modular
fn process_bot_actions(app: &mut App) {
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
            
            if app.game.round != game::Round::PreFlop && app.game.community_cards.is_empty() {
                app.messages.push(format!("Dealing cards for {:?} round", app.game.round));
                app.game.deal_community_cards();
            }
            
            // Get bot action
            match app.game.get_bot_action(bot_player) {
                Ok(bot_action) => {
                    // Process the bot's action
                    app.process_bot_action(bot_action, bot_player.clone());
                },
                Err(e) => {
                    app.messages.push(format!("Bot error: {}", e));
                    // End the game on error to prevent loops
                    app.game_active = false;
                }
            }
        }
    }
}