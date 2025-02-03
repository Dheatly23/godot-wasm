mod board;
mod robot;

use board::*;
use robot::{DummyRobot, Robot};

static mut BOARD: Board = Board::new_empty();
static mut ROBOT: Option<Box<dyn Robot>> = None;

#[unsafe(no_mangle)]
pub extern "C" fn init(w: u64, h: u64) {
    unsafe {
        BOARD = Board::new(w as _, h as _);
        ROBOT = Some(<Box<DummyRobot>>::default());
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn make_move(player: u64) -> u64 {
    let board = unsafe { &mut *(&raw mut BOARD) };

    let mut x = player as usize;
    let mut y = board.get_move(x).expect("Invalid move");
    board[(x, y)] = CellState::Player;

    x = unsafe {
        (*(&raw mut ROBOT))
            .as_mut()
            .unwrap()
            .make_move(&*board, (x, y))
    };
    y = board.get_move(x).expect("Invalid move");
    board[(x, y)] = CellState::Robot;

    x as _
}
