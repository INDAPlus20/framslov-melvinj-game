pub mod structs;
pub use structs::*;
fn main() {
	#[no_mangle]
	pub extern "C" fn add(a: isize, b: isize) -> isize {
		a + b
	}
	#[no_mangle]
	pub extern "C" fn calculate_move(game: &GameState) -> InputState {
		InputState {
            xaxis1pos: 1.0,
            xaxis1neg: 0.0,
            yaxis1pos: 0.0,
            yaxis1neg: 0.0,
            xaxis2pos: 0.0,
            xaxis2neg: 0.0,
            yaxis2pos: 0.0,
            yaxis2neg: 0.0,
        }
	}
}
