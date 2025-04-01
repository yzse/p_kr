# p_kr

A text-based poker game with AI opponents on a text-only interface.

## Features

- Simple TUI (Text User Interface) poker game
- Play against AI opponents with adjustable difficulty levels
- Text-only card representation
- Integration with OpenAI's GPT for AI decision-making

## Requirements

- Rust and Cargo (install from https://rustup.rs if not already installed)

## Setup

1. Clone this repository
2. Set your OpenAI API key (optional for full functionality):
   ```
   export OPENAI_API_KEY=your_api_key_here
   ```

## Running the Game

```
cargo run
```

## Game Controls

- `d`: Deal a new hand
- `c`: Call the current bet
- `k`: Check (when no bet to call)
- `f`: Fold your hand
- `r`: Raise (enter a number first, then press 'r')
- `q`: Quit the game

## Note

Without an OpenAI API key, the game will simulate AI decisions based on difficulty levels.