use crate::game::GameState;
use rand::seq::SliceRandom;
use std::fmt;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

// Default UBC1 exploration constant. Equals sqrt(2).
pub const UCB1_DEFAULT_EXPLORE_CONST: f64 = 1.41421356237309504880168872420;

pub enum SelectionPolicy {
    Ucb1(Option<f64>),
}

#[derive(Clone)]
struct Node<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    /// The move that got the game state to this node.
    mv: Option<M>,
    /// The ID of the parent node, or None if this is the root node.
    parent_node: Option<usize>,
    /// The IDs of the child nodes.
    child_nodes: Vec<usize>,
    /// The number of wins the player just moved has from this node.
    /// Specifically, the previous player in the game state is used.
    wins: u64,
    /// The number of times this node has been rolled out from.
    visits: u64,
    /// The vec of untried moves that are still available.
    untried_mvs: Vec<M>,
    /// The game state that this node reflects.
    state: S, // TODO [mem]: Move to Option<Box> & drop once done with.
    // Required members due to odd generic params.
    _phantom_p: PhantomData<P>,
    _phantom_me: PhantomData<ME>,
}

impl<P, M, ME, S> Node<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    /// Constructs a new node using the given setup data.
    pub fn new(mv: Option<M>, parent_node: Option<usize>, state: S) -> Self {
        Node {
            mv,
            parent_node,
            child_nodes: vec![],
            wins: 0,
            visits: 0,
            untried_mvs: state.get_moves(),
            state,
            _phantom_p: PhantomData,
            _phantom_me: PhantomData,
        }
    }

    /// Returns whether this node is fully expanded or not.
    /// If false, then more children can be added.
    pub fn is_fully_expanded(&self) -> bool {
        self.untried_mvs.len() == 0
    }

    /// Returns whether this node has any children.
    pub fn has_children(&self) -> bool {
        self.child_nodes.len() != 0
    }

    /// Updates the visits & wins counts based on the given winner.
    pub fn update(&mut self, winner: Option<P>) -> () {
        self.visits += 1;

        if let Some(wnr) = winner {
            if wnr == self.state.get_prev_player() {
                self.wins += 1;
            }
        }
    }
}

pub struct Mcts<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    /// The node tree.
    tree: Vec<Node<P, M, ME, S>>,
    /// The ID of the current root node in the tree vec.
    cur_node_id: usize,
    /// The player that we are working for. This is mostly for checking purposes.
    target_player: P,
}

impl<P, M, ME, S> Mcts<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    /// Constructs a new Mcts object given the player and initial state.
    pub fn new(target_player: P, orig_state: &S) -> Self {
        let mut mcts = Mcts {
            tree: vec![],
            cur_node_id: Default::default(),
            target_player,
        };
        mcts.cur_node_id = mcts.push_node(Node::new(None, None, orig_state.clone()));
        mcts
    }

    /// Updates the root node to reflect an opponent's move.
    pub fn update_opponent_move(&mut self, mv: M) -> () {
        self.update_move(mv, false);
    }

    /// Updates the root node to reflect the target player's move.
    pub fn update_target_move(&mut self, mv: M) -> () {
        self.update_move(mv, true);
    }

    /// Runs MCTS to select the next best move until the compute limit is reached.
    /// Once this limit is reached, the best move is selected & returned, along
    /// with the number of rounds that were performed within the limit.
    pub fn select_next_move(
        &mut self,
        compute_limit: Duration,
        selection_pol: &SelectionPolicy,
    ) -> (M, u64) {
        // Prune out nodes we don't need.
        self.prune_nodes();

        let start = Instant::now();
        let mut rounds = 0;
        while Instant::now() - start < compute_limit {
            let mut node = self.phase_selection(self.cur_node_id, selection_pol);
            node = self.phase_expansion(node);
            let winner = self.phase_rollout(&self.get_node(node).state);
            self.phase_backprop(node, winner);
            rounds += 1;
        }

        (self.phase_action_select(), rounds)
    }

    // General helper fns.

    /// Updates the root node to match to move that was performed. Does some
    /// quality-of-life checks to ensure we are working with the right player.
    fn update_move(&mut self, mv: M, for_target_player: bool) -> () {
        let tgt = self.target_player;
        let node = self.get_cur_node();

        // Ensure that we are working with the right player.
        let target_is_current = tgt == node.state.get_current_player();
        if for_target_player && !target_is_current {
            panic!("Updating move for target player but on opponent");
        } else if !for_target_player && target_is_current {
            panic!("Updating move for opponent but on target player");
        }

        // Attempt to find a child node from the root that matches the move that
        // has been performed.
        let mut next_id: Option<usize> = None;
        for (id, child) in node
            .child_nodes
            .iter()
            .map(|&child_id| (child_id, self.get_node(child_id)))
        {
            if let Some(m) = child.mv {
                if m == mv {
                    next_id = Some(id);
                }
            }
        }

        match next_id {
            // Update the current root node to the found child node.
            Some(child_id) => self.cur_node_id = child_id,
            // Create a child node from the root & make them the new root.
            None => self.cur_node_id = self.make_move(self.cur_node_id, mv),
        };
    }

    /// From the given node, creates a child node that represents the given move
    /// & return the ID of the new node.
    fn make_move(&mut self, node_id: usize, mv: M) -> usize {
        let state: S;

        // Prevent double mut borrow using nested scope.
        {
            let node = self.get_node_mut(node_id);
            node.untried_mvs.retain(|&m| m != mv);
            state = node.state.from_move(mv).unwrap();
        }

        let child_id = self.push_node(Node::new(Some(mv), Some(node_id), state));
        self.get_node_mut(node_id).child_nodes.push(child_id);
        child_id
    }

    /// Pushes the given node onto the tree & returns the ID of it.
    fn push_node(&mut self, node: Node<P, M, ME, S>) -> usize {
        let id = self.tree.len();
        self.tree.push(node);
        id
    }

    /// Prunes out all nodes that aren't decentants of the current root node.
    ///
    /// # Notes
    ///
    /// This method will make a complete copy of the node tree with only the
    /// required nodes in, meaning that it shouldn't be done in time-critical
    /// sections.
    fn prune_nodes(&mut self) -> () {
        let mut cur_node = self.get_cur_node().clone();
        cur_node.parent_node = None;
        let mut n_tree = vec![cur_node];

        // Recursively append children to new tree.
        self.append_children_to(0, &mut n_tree);

        // Once done, replace old tree & update current node.
        self.tree = n_tree;
        self.cur_node_id = 0;
    }

    /// Appends all of a node's children from the old tree onto the new tree.
    /// This method will work recursively with all children & sub-children.
    fn append_children_to(&self, c_id: usize, n_tree: &mut Vec<Node<P, M, ME, S>>) {
        let children = n_tree[c_id].child_nodes.clone();
        let mut n_children = vec![];

        // Copy all children from the current node over to the new tree.
        for &child_id in children.iter() {
            n_children.push(n_tree.len());
            let mut child = self.get_node(child_id).clone();
            child.parent_node = Some(c_id);
            n_tree.push(child);
        }

        // For each child, append their children to the new tree.
        // We do this after just to keep all the children of a node together in
        // a single block.
        for &child_id in n_children.iter() {
            self.append_children_to(child_id, n_tree);
        }

        // Update the child nodes vec with the new IDs.
        n_tree[c_id].child_nodes = n_children;
    }

    // Phase fns.

    /// Selection phase of MCTS. Selects the next child to work on & returns
    /// the node's ID.
    fn phase_selection(&self, node_id: usize, selection_pol: &SelectionPolicy) -> usize {
        let node = self.get_node(node_id);

        if !node.is_fully_expanded() || !node.has_children() {
            node_id
        } else {
            let child_id = self.select_max_child(
                node,
                match selection_pol {
                    SelectionPolicy::Ucb1(expl) => {
                        let ex = expl.unwrap_or(UCB1_DEFAULT_EXPLORE_CONST);
                        move |child| self.selector_ucb1(node, child, ex)
                    }
                },
            );

            self.phase_selection(child_id, selection_pol)
        }
    }

    /// Expansion phase of MCTS. Selects a move at random to perform from the
    /// given node, and creates a child node representing that move. The ID of
    /// the child is then returned.
    ///
    /// If no move can be done, then the given node ID itself is returned. In
    /// this case, it means that the node is at the end of the game.
    fn phase_expansion(&mut self, node_id: usize) -> usize {
        match self
            .get_node_mut(node_id)
            .untried_mvs
            .choose(&mut rand::thread_rng())
        {
            Some(&mv) => self.make_move(node_id, mv),
            None => node_id,
        }
    }

    /// Rollout phase of MCTS. Performs a completely random game to completion
    /// & returns the winner of that game.
    fn phase_rollout(&self, state: &S) -> Option<P> {
        let mut working_state = state.clone();
        while let Some(&mv) = working_state.get_moves().choose(&mut rand::thread_rng()) {
            working_state.make_move(mv).unwrap();
        }

        working_state.get_winner()
    }

    /// Backprop phase of MCTS. Updates the current node and all parents with
    /// the winner of the rollout phase.
    fn phase_backprop(&mut self, node_id: usize, winner: Option<P>) -> () {
        let mut current_node = self.get_node_mut(node_id);
        loop {
            current_node.update(winner);

            match current_node.parent_node {
                Some(parent_node_id) => current_node = self.get_node_mut(parent_node_id),
                None => break,
            }
        }
    }

    /// Action selection phase of MCTS. Selects the move with the best chance of
    /// winning from the current root node.
    fn phase_action_select(&self) -> M {
        let child_id = self.select_max_child(self.get_cur_node(), |child| {
            (child.wins as f64) / (child.visits as f64)
        });
        self.get_node(child_id).mv.unwrap()
    }

    // Phase helper fns.

    /// Returns the ID of the child node that scored highest on some given
    /// predicate.
    fn select_max_child<'a, F: FnMut(&'a Node<P, M, ME, S>) -> f64>(
        &'a self,
        node: &'a Node<P, M, ME, S>,
        mut selector: F,
    ) -> usize {
        let mut children = node
            .child_nodes
            .iter()
            .map(|&child_id| (child_id, self.get_node(child_id)))
            .collect::<Vec<(usize, &Node<P, M, ME, S>)>>();
        children.sort_by(|&(_, x), &(_, y)| selector(x).partial_cmp(&selector(y)).unwrap());
        children.last().unwrap().0
    }

    /// The standard UCB1 selector function.
    fn selector_ucb1(
        &self,
        node: &Node<P, M, ME, S>,
        child: &Node<P, M, ME, S>,
        explore_const: f64,
    ) -> f64 {
        (child.wins as f64) / (child.visits as f64)
            + explore_const * ((node.visits as f64).ln() / (child.visits as f64)).sqrt()
    }

    // Util fns.

    /// Returns a reference to the current root node.
    fn get_cur_node(&self) -> &Node<P, M, ME, S> {
        self.get_node(self.cur_node_id)
    }

    /// Returns a reference to the given node.
    fn get_node(&self, node_id: usize) -> &Node<P, M, ME, S> {
        &self.tree[node_id]
    }

    /// Returns a mutable reference to the given node.
    fn get_node_mut(&mut self, node_id: usize) -> &mut Node<P, M, ME, S> {
        &mut self.tree[node_id]
    }
}
