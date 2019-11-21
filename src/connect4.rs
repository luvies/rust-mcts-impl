use crate::game::GameState;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Player {
    Red,
    Yellow,
}

impl Player {
    pub fn all() -> Vec<Self> {
        vec![Self::Red, Self::Yellow]
    }

    pub fn next(self) -> Self {
        match self {
            Self::Red => Player::Yellow,
            Self::Yellow => Player::Red,
        }
    }
}

impl ToString for Player {
    fn to_string(&self) -> String {
        match self {
            Self::Red => "R".to_owned(),
            Self::Yellow => "Y".to_owned(),
        }
    }
}

pub type Move = u8;

#[derive(Clone, Copy)]
pub enum MoveError {
    OutOfRange(Move),
    ColumnFull(Move),
}

impl fmt::Debug for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(mv) => write!(f, "Move {} is out of range", mv),
            Self::ColumnFull(mv) => write!(f, "Column {} is full", mv),
        }
    }
}

const WIDTH: usize = 7;
const HEIGHT: usize = 6;
const CONNECT_LEN: usize = 4;

// TODO: Move to const generics.
#[derive(Clone, Debug)]
pub struct Game {
    turn: Player,
    board: [[Option<Player>; HEIGHT]; WIDTH],
    winner: Option<Player>,
}

#[derive(Clone, Copy)]
struct Point(usize, usize);
#[derive(Clone, Copy)]
struct PointDirection(i64, i64);

impl Game {
    pub fn new() -> Self {
        Game {
            turn: Player::Red,
            board: [[None; HEIGHT]; WIDTH],
            winner: None,
        }
    }

    fn update_winner_from(&mut self, col: usize, row: usize) -> () {
        if let Some(ply) = self.board[col][row] {
            for &dir in &[
                PointDirection(1, 0),
                PointDirection(1, 1),
                PointDirection(0, 1),
                PointDirection(-1, 1),
            ] {
                let start = Point(col, row);
                let count = 1
                    + self.count_line_from(start, dir, ply, false)
                    + self.count_line_from(start, dir, ply, true);

                if count >= CONNECT_LEN as u64 {
                    self.winner = Some(ply);
                }
            }
        }
    }

    fn count_line_from(&self, start: Point, dir: PointDirection, player: Player, rev: bool) -> u64 {
        let mut count = 0;
        for dist in 1..(CONNECT_LEN as i64) {
            if let Some(Point(col, row)) = Self::get_point_from(start, dir, dist, rev) {
                if let Some(ply) = self.board[col][row] {
                    if ply == player {
                        count += 1;
                        continue;
                    }
                }
            }
            break;
        }

        count
    }

    fn get_point_from(start: Point, dir: PointDirection, dist: i64, rev: bool) -> Option<Point> {
        let Point(col_i, row_i) = start;
        let PointDirection(col_d, row_d) = dir;

        let c_d = col_d as i64 * dist;
        let r_d = row_d as i64 * dist;

        let n_col: i64;
        let n_row: i64;
        if rev {
            n_col = col_i as i64 - c_d;
            n_row = row_i as i64 - r_d;
        } else {
            n_col = col_i as i64 + c_d;
            n_row = row_i as i64 + r_d;
        }

        if n_col >= 0 && n_col < WIDTH as i64 && n_row >= 0 && n_row < HEIGHT as i64 {
            Some(Point(n_col as usize, n_row as usize))
        } else {
            None
        }
    }
}

impl GameState<Player, Move, MoveError> for Game {
    fn make_move(&mut self, mv: Move) -> Result<(), MoveError> {
        let col_i = mv as usize;
        if col_i >= WIDTH {
            return Err(MoveError::OutOfRange(mv));
        }

        let ref mut col = self.board[col_i];
        match col.iter().position(|&cell| cell == None) {
            Some(row_i) => {
                col[row_i] = Some(self.turn);
                self.update_winner_from(col_i, row_i);
                self.turn = self.turn.next();
                Ok(())
            }
            None => Err(MoveError::ColumnFull(mv)),
        }
    }

    fn get_moves(&self) -> Vec<Move> {
        match self.get_winner() {
            Some(_) => vec![],
            None => self
                .board
                .iter()
                .zip(0..WIDTH) // Zip in index.
                .filter(|(col, _)| match col.last() {
                    Some(None) => true,
                    _ => false,
                })
                .map(|(_, i)| i as Move)
                .collect(),
        }
    }

    fn get_winner(&self) -> Option<Player> {
        self.winner
    }

    fn get_current_player(&self) -> Player {
        self.turn
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = "".to_owned();
        for row in (0..HEIGHT).rev() {
            out = format!("{}{}", out, "|");
            for col in 0..WIDTH {
                out = format!(
                    "{} {} |",
                    out,
                    match self.board[col][row] {
                        Some(ply) => ply.to_string(),
                        None => " ".to_owned(),
                    }
                )
            }
            out = format!(
                "{}{}",
                out,
                match row {
                    r if r > 0 => "\n",
                    _ => "",
                }
            );
        }

        write!(
            f,
            "{}\n{}",
            out,
            (0..(WIDTH * 4))
                .map(|_| "-")
                .fold("-".to_owned(), |a, b| format!("{}{}", a, b))
        )
    }
}
