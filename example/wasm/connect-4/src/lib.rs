mod board;
mod robot;

extern crate wee_alloc;

use board::*;
use robot::{DummyRobot, Robot};

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

static mut BOARD: Board = Board::new_empty();
static mut ROBOT: Option<Box<dyn Robot>> = None;

#[no_mangle]
pub extern "C" fn init(w: u64, h: u64) {
    unsafe {
        BOARD = Board::new(w as _, h as _);
        ROBOT = Some(Box::new(DummyRobot::default()));
    }
}

#[no_mangle]
pub extern "C" fn make_move(player: u64) -> u64 {
    let board = unsafe { &mut BOARD };

    let mut x = player as usize;
    let mut y = board.get_move(x).expect("Invalid move");
    board[(x, y)] = CellState::Player;

    x = unsafe { ROBOT.as_mut().unwrap().make_move(&*board, (x, y)) };
    y = board.get_move(x).expect("Invalid move");
    board[(x, y)] = CellState::Robot;

    return x as _;
}
