# framslov-melvinj-game: Space Dodgeball
## How to write script:
1. Write it in rust
2. Place it in the src/script - folder. Name it <whatever except 'structs'>.rs
2. Follow the template provided in src/script/wacko_ai.rs and below:
```rust
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
            xaxis1pos: 0.0,
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
```
3. Make sure it compiles properly. Errors will mean the game does not start
4. DO NOT remove the add-function. Very important for testing if the script is OK.
5. All structs are in the src/script/structs.rs file
6. I have not been able to use any external libraries/dependencies in the script. Try at your own will
7. The only script which will run is the last script found in src/script - folder. Move wacko_ai.rs if neccesary.

We are planning on adding PvP, PvAI, AIvAI
We are planning on having a menu for selecting scripts
