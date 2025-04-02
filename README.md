# p_kr

A text-based poker game with AI opponents on a text-only interface.

<img width="714" alt="Screenshot 2025-04-01 at 10 59 46 PM" src="https://github.com/user-attachments/assets/e5963fe8-94c2-4221-b4c4-3d148ac7bd2e" />

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

