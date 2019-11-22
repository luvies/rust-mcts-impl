# MCTS Rust Implementation

This is an implementation of Monte Carlo Tree Search in rust. The algorithm itself only relies on the [GameState](src/game.rs) trait, meaning any game that implements that trait can be used.

Currently, only [connect 4](src/connect4.rs) is implemented, and this is the game that is used to test the MCTS implementation.

As this is only a proof-of-concept, this repo isn't built as a crate, meaning that it isn't on crate.io. However, if you wanted to actually use an MCTS crate, then I'm sure there's one already that suits your requirements.
