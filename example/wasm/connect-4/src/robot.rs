use super::board::{Board, CellState};

pub trait Robot {
    fn make_move(&mut self, board: &Board, player_cell: (usize, usize)) -> usize;
}

#[derive(Default)]
pub struct DummyRobot {
    ix: usize,
}

impl Robot for DummyRobot {
    fn make_move(&mut self, board: &Board, _: (usize, usize)) -> usize {
        loop {
            self.ix = (self.ix + 1) % board.width();
            if board[(self.ix, board.height() - 1)] == CellState::Empty {
                break self.ix;
            }
        }
    }
}
