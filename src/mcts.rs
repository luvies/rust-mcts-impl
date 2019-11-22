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

// TODO [mem]: Drop nodes that are no longer part of the game tree.
struct Node<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    mv: Option<M>,
    parent_node: Option<usize>,
    child_nodes: Vec<usize>,
    wins: u64,
    visits: u64,
    untried_mvs: Vec<M>,
    state: S, // TODO [mem]: Move to Option<Box> & drop once done with.
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

    pub fn is_fully_expanded(&self) -> bool {
        self.untried_mvs.len() == 0
    }

    pub fn has_children(&self) -> bool {
        self.child_nodes.len() != 0
    }

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
    tree: Vec<Node<P, M, ME, S>>,
    cur_node_id: usize,
    target_player: P,
}

impl<P, M, ME, S> Mcts<P, M, ME, S>
where
    P: Copy + PartialEq + ToString + fmt::Debug,
    M: Copy + PartialEq + fmt::Debug,
    ME: Copy + fmt::Debug,
    S: GameState<P, M, ME>,
{
    pub fn new(target_player: P, orig_state: &S) -> Self {
        let mut mcts = Mcts {
            tree: vec![],
            cur_node_id: Default::default(),
            target_player,
        };
        mcts.cur_node_id = mcts.push_node(Node::new(None, None, orig_state.clone()));
        mcts
    }

    pub fn update_opponent_move(&mut self, mv: M) -> () {
        self.update_move(mv, false);
    }

    pub fn update_target_move(&mut self, mv: M) -> () {
        self.update_move(mv, true);
    }

    pub fn select_next_move(
        &mut self,
        compute_limit: Duration,
        selection_pol: &SelectionPolicy,
    ) -> (M, u64) {
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

    fn update_move(&mut self, mv: M, for_target_player: bool) -> () {
        let tgt = self.target_player;
        let node = self.get_cur_node();

        let target_is_current = tgt == node.state.get_current_player();
        if for_target_player && !target_is_current {
            panic!("updating move for target player but on opponent");
        } else if !for_target_player && target_is_current {
            panic!("updating move for target player but on opponent");
        }

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
            Some(child_id) => self.cur_node_id = child_id,
            None => self.cur_node_id = self.make_move(self.cur_node_id, mv),
        };
    }

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

    fn push_node(&mut self, node: Node<P, M, ME, S>) -> usize {
        let id = self.tree.len();
        self.tree.push(node);
        id
    }

    // Phase fns.

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

    fn phase_rollout(&self, state: &S) -> Option<P> {
        let mut working_state = state.clone();
        while let Some(&mv) = working_state.get_moves().choose(&mut rand::thread_rng()) {
            working_state.make_move(mv).unwrap();
        }

        working_state.get_winner()
    }

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

    fn phase_action_select(&self) -> M {
        let child_id = self.select_max_child(self.get_cur_node(), |child| {
            (child.wins as f64) / (child.visits as f64)
        });
        self.get_node(child_id).mv.unwrap()
    }

    // Phase helper fns.

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

    fn get_cur_node(&self) -> &Node<P, M, ME, S> {
        self.get_node(self.cur_node_id)
    }

    fn get_node(&self, node_id: usize) -> &Node<P, M, ME, S> {
        &self.tree[node_id]
    }

    fn get_node_mut(&mut self, node_id: usize) -> &mut Node<P, M, ME, S> {
        &mut self.tree[node_id]
    }
}
