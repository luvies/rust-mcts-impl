use std::fmt;

/// Basic game info for MCTS to query game state.
pub trait GameState<P, M, ME>: Clone + fmt::Display + fmt::Debug
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
{
    /// Returns a new game state that has the given move performed.
    fn from_move(&self, mv: M) -> Result<Self, ME> {
        let mut new_state = self.clone();
        new_state.make_move(mv)?;
        Ok(new_state)
    }

    /// Mutates the current game state with the new move.
    fn make_move(&mut self, mv: M) -> Result<(), ME>;
    /// Returns the available moves of the current player.
    fn get_moves(&self) -> Vec<M>;
    /// Returns the current winner.
    fn get_winner(&self) -> Option<P>;
    /// Returns the current player.
    fn get_current_player(&self) -> P;
}
