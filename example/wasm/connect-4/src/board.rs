use std::ops;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellState {
    Empty,
    Player,
    Robot,
}

impl Default for CellState {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Default)]
pub struct Board {
    board: Vec<CellState>,
    width: usize,
    height: usize,
}

impl Board {
    pub const fn new_empty() -> Self {
        Self {
            board: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    pub fn new(width: usize, height: usize) -> Self {
        let mut board = Vec::new();
        board.resize_with(width * height, CellState::default);
        Self {
            board,
            width,
            height,
        }
    }

    #[inline(always)]
    pub fn width(&self) -> usize {
        self.width
    }

    #[inline(always)]
    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_move(&self, x: usize) -> Option<usize> {
        let mut y = self.height - 1;
        while (y > 0) && (self[(x, y)] == CellState::Empty) {
            y -= 1;
        }

        y += 1;
        if y == self.height { None } else { Some(y) }
    }
}

impl ops::Index<(usize, usize)> for Board {
    type Output = CellState;

    fn index(&self, (x, y): (usize, usize)) -> &Self::Output {
        if (x >= self.width) || (y >= self.height) {
            panic!("Index out of bounds!");
        }
        &self.board[x * self.height + y]
    }
}

impl ops::IndexMut<(usize, usize)> for Board {
    fn index_mut(&mut self, (x, y): (usize, usize)) -> &mut Self::Output {
        if (x >= self.width) || (y >= self.height) {
            panic!("Index out of bounds!");
        }
        &mut self.board[x * self.height + y]
    }
}
