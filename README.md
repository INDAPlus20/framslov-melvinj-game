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
	//Write script here
	
	//Use the GameState to generate your InputState for the ship you are controlling
	
	
        if game.player1.pos.0 < -10.0 {
		    InputState {
                xaxis1pos: 1.0,
                xaxis1neg: 0.0,
                yaxis1pos: 0.0,
                yaxis1neg: 0.0,
                xaxis2pos: 0.0,
                xaxis2neg: 0.0,
                yaxis2pos: 0.0,
                yaxis2neg: 0.0,
                holdball: true,
            }
        } else {
            InputState {
                xaxis1pos: 1.0,
                xaxis1neg: 0.0,
                yaxis1pos: 0.0,
                yaxis1neg: 0.0,
                xaxis2pos: 0.0,
                xaxis2neg: 0.0,
                yaxis2pos: 0.0,
                yaxis2neg: 0.0,
                holdball: false,
            }
        }

    }
}

```
3. Make sure it compiles properly. Errors in compilation will mean the game does not start OR cause terrible bugs / crashes. If it does not compile, the game will not start OR use a previously compiled script, if such exists.
4. DO NOT remove the add-function. Very important for testing if the script is OK.
5. All structs are in the src/script/structs.rs file
6. I have not been able to use any external libraries/dependencies in the script. Tell me if you can make it work
7. Run your script in the command line as you would the game itself, but with 0-2 arguments.
```cargo run wacko_ai``` will be AIvP (no second arg ==> manual p2)
```cargo run scriptnamenotinscriptsfolder wacko_ai``` will be PvAI. (first arg invalid ==> manual p1)
Any argument which is not a part of any file name will result in that player becoming player-controlled

We are planning on letting the script know which player it is controlling.
We are planning on having a menu for selecting scripts
