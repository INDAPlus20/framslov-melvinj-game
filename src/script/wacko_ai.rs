/// **********************************************************************
/// Note: Scripts should only affect one player each
/// Only change either '1' or '2' values to avoid cheating
/// AIs may access the position and velocities of all PhysObjects in the game
/// **********************************************************************

pub mod structs;
pub use structs::*;
fn main() {

	#[no_mangle]
	pub extern "C" fn add(a: isize, b: isize) -> isize {
		a + b
	}
	#[no_mangle]
	pub extern "C" fn calculate_move(game: &GameState, p1: bool) -> InputState {
        if p1 {
            if game.player1.pos.0 < -10.0 {
                InputState {
                    xaxis1pos: 1.0,
                    xaxis1neg: 0.0,
                    yaxis1pos: 0.0,
                    yaxis1neg: 0.0,
                    holdball: true,
                }
            } else {
                InputState {
                    xaxis1pos: 1.0,
                    xaxis1neg: 0.0,
                    yaxis1pos: 0.0,
                    yaxis1neg: 0.0,
                    holdball: false,
                }
            }
        } else {
            if game.player2.pos.0 > 10.0 {
                InputState {
                    xaxis1pos: 0.0,
                    xaxis1neg: -1.0,
                    yaxis1pos: 0.0,
                    yaxis1neg: 0.0,
                    holdball: true,
                }
            } else {
                InputState {
                    xaxis1pos: 0.0,
                    xaxis1neg: -1.0,
                    yaxis1pos: 0.0,
                    yaxis1neg: 0.0,
                    holdball: false,
                }
            }
        }

    }
}
