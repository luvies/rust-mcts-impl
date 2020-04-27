extern crate rand;
mod connect4;
mod game;
mod mcts;

use game::GameState;
use mcts::{Mcts, SelectionPolicy};
use std::fmt;
use std::time::Duration;

/// Runs a game where all players are AIs based on MCTS.
fn do_ai_game<P, M, ME, S>(
    state: &mut S,
    players: Vec<P>,
    compute_limit: Duration,
    selection_pol: SelectionPolicy,
) where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    let mut cur_ply = 0;
    let mut ais: Vec<Mcts<P, M, ME, S>> =
        players.iter().map(|&ply| Mcts::new(ply, state)).collect();

    println!("{}", state);

    while !state.get_moves().is_empty() {
        let (mv, rounds) = ais[cur_ply].select_next_move(compute_limit, &selection_pol);
        state.make_move(mv).unwrap();

        for (i, ai) in ais.iter_mut().enumerate() {
            if i == cur_ply {
                ai.update_target_move(mv);
            } else {
                ai.update_opponent_move(mv);
            }
        }

        println!("{}\n{} rounds of MCTS", state, rounds);

        cur_ply += 1;
        cur_ply %= ais.len();
    }

    println!(
        "Game ended, winner: {}",
        match state.get_winner() {
            Some(ply) => ply.to_string(),
            None => "None".to_owned(),
        }
    );
}

fn main() {
    let mut state = connect4::Game::new();
    do_ai_game(
        &mut state,
        connect4::Player::all(),
        Duration::from_millis(1000),
        SelectionPolicy::Ucb1(None),
    );
}
