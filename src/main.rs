
//! An Asteroids-ish example game to show off ggez.
//! The idea is that this game is simple but still
//! non-trivial enough to be interesting.

use ggez;
// use ggez::audio;
// use ggez::audio::SoundSource;
use ggez::conf;
use ggez::event::{self, EventHandler, KeyCode, KeyMods};
use ggez::graphics;
use ggez::nalgebra as na;
use ggez::timer;
use ggez::{Context, ContextBuilder, GameResult};

use std::env;
use std::path;
use std::cell::Cell;

// use std::fs::File;
// use std::io::prelude::*;
use std::process::Command;
use libloading::{Library, Symbol};
use std::fs::{self};
use std::path::Path;

type AddFunc = unsafe fn(isize, isize) -> isize;
type AIFunc = unsafe fn(&GameState, bool) -> InputState;

type Point2 = na::Point2<f32>;

/// *********************************************************************
/// Now we define our PhysObjects.
/// A PhysObject is anything in the game world.
/// **********************************************************************
#[derive(Debug)]
enum PhysType {
    Player,
    Ball
}

#[derive(Debug)]
struct PhysObject {
    tag: PhysType,
    id: f32,
    hold: f32,
    pos: (f32, f32),
    x_velocity: f32,
    y_velocity: f32,
    bbox_size: f32,
}

// We use cells to allow each ball a unique ID
thread_local!(static BALL_ID: Cell<f32> = Cell::new(2.0));

impl PhysObject {
    fn new_ball_id(pos: (f32, f32)) -> PhysObject {
        BALL_ID.with(|thread_id| {
            let id = thread_id.get();
            thread_id.set(id + 1.0);
            PhysObject {
                tag: PhysType::Ball,
                id,
                hold: 0.0,
                pos,
                x_velocity: 0.0,
                y_velocity: 0.0,
                bbox_size: ROCK_BBOX
            }
        })
    }

    fn get_pos_p2(&self) -> Point2 {
        Point2::new(self.pos.0, self.pos.1)
    }
}

const PLAYER_BBOX: f32 = 24.0;
const ROCK_BBOX: f32 = 24.0;

/// *********************************************************************
/// Now we have some constructor functions for different PhysObject.
/// **********************************************************************

fn create_player(spawn_pos: (f32, f32), player_id: f32) -> PhysObject {
    PhysObject {
        tag: PhysType::Player,
        id: player_id,
        hold: 0.0,
        pos: spawn_pos,
        x_velocity: 0.0,
        y_velocity: 0.0,
        bbox_size: PLAYER_BBOX
    }
}

fn create_balls(balls_num: f32) -> Vec<PhysObject> {
    let mut balls = Vec::new();
    let distance = 100.0;
    balls.append(&mut create_balls_collumn((balls_num / 2.0).ceil(), distance));
    balls.append(&mut create_balls_collumn((balls_num / 2.0).floor(), distance + 72.0));

    return balls;
}

fn create_balls_collumn(balls_num: f32, distance: f32) -> Vec<PhysObject> {
    let space = 72.0;
    let mut space_iter = -((balls_num - 1.0) * space) / 2.0;
    let mut balls = Vec::new();
    for _ in 0..balls_num as i32 {
        balls.append(&mut create_ball_pair(distance, space_iter));
        space_iter += space;
    }

    return balls;
}

fn create_ball_pair(x: f32, y: f32) -> Vec<PhysObject> {
    let mut balls = Vec::new();
    balls.push(PhysObject::new_ball_id((-x, y)));
    balls.push(PhysObject::new_ball_id((x, y)));
    
    return balls;
}

fn reset_field(width: f32) -> (PhysObject, PhysObject, Vec<PhysObject>) {
    let player1 = create_player((-3.0 * width / 8.0, 0.0), 1.0);
    let player2 = create_player((3.0 * width / 8.0, 0.0), 2.0);
    let balls = create_balls(6.0); 
    return (player1, player2, balls);
}

fn ball_id_to_elem(balls: &Vec<PhysObject>, id: f32) -> Option<usize> {
    for (index, ball) in balls.iter().enumerate() {
        if ball.id == id {
            return Some(index);
        }
    }
    return None;
}

fn ball_follow(player: &PhysObject, balls: &mut Vec<PhysObject>, offset: f32) {
    let index = ball_id_to_elem(&balls, player.hold);
    match index {
        Some(x) => {
            balls[x].pos.0 = player.pos.0 + offset;
            balls[x].pos.1 = player.pos.1;
            balls[x].hold = player.id;
        },
        _ => ()
    }
}

/// *********************************************************************
/// Now we make functions to handle physics. We do simple Newtonian
/// physics (so we do have inertia), and cap the max speed so that we
/// don't have to worry too much about the insane levels of power a 
/// player could reach.
///
/// Our unit of world space is simply pixels, though we do transform
/// the coordinate system so that +y is up and -y is down.
/// **********************************************************************

/// Acceleration in pixels per second squared.
const PLAYER_ACCELERATION: f32 = 8.0;
/// Max velocity in pixels per second
const MAX_PHYSICS_VEL: f32 = 200.0;
/// Deacceleration in pixels per second squared.
const BALL_DRAG: f32 = 20.0;

fn player_handle_input(player: &mut PhysObject, input: &InputState, balls: &mut Vec<PhysObject>) {
    player.x_velocity += PLAYER_ACCELERATION * (input.xaxis1pos + input.xaxis1neg);
    player.y_velocity += PLAYER_ACCELERATION * (input.yaxis1pos + input.yaxis1neg);
    if player.hold == 0.0 && input.holdball {
        ball_pickup(player, balls);
    } else if player.hold != 0.0 && !input.holdball {
        ball_drop(player, balls);
    }
}

fn ball_halt(ball: &mut PhysObject, dt: f32) {
    if ball.x_velocity.abs().floor() != 0.0 || ball.y_velocity.abs().floor() != 0.0 {
        let pythagoras = (ball.x_velocity.powf(2.0) + ball.y_velocity.powf(2.0)).powf(0.5);
        ball.x_velocity -= BALL_DRAG * ball.x_velocity.signum() * dt * ball.x_velocity.abs() / pythagoras;
        ball.y_velocity -= BALL_DRAG * ball.y_velocity.signum() * dt * ball.y_velocity.abs() / pythagoras;
    }
    else {
        ball.x_velocity = 0.0;
        ball.y_velocity = 0.0;
        ball.hold = 0.0;
    }
}

fn update_object_position(object: &mut PhysObject, width_lower: f32, width_upper: f32, height: f32, dt: f32) {
    // Clamp the velocity to the max *efficiently*

    if object.x_velocity.abs() > MAX_PHYSICS_VEL {
        object.x_velocity = object.x_velocity.signum() * MAX_PHYSICS_VEL;
    }
    if object.y_velocity.abs() > MAX_PHYSICS_VEL {
        object.y_velocity = object.y_velocity.signum() * MAX_PHYSICS_VEL;
    }

    let dxv = object.x_velocity * dt;
    let dyv = object.y_velocity * dt;

    if object.pos.0 + dxv < width_lower {
        object.pos.0 = 2.0 * width_lower - (object.pos.0 + dxv);
        object.x_velocity *= -1.0;
    }
    else if object.pos.0 + dxv > width_upper {
        object.pos.0 = 2.0 * width_upper - (object.pos.0 + dxv);
        object.x_velocity *= -1.0;
    }
    else {
        object.pos.0 += dxv;
    }

    if object.pos.1 + dyv < height / -2.0 {
        object.pos.1 = -height - (object.pos.1 + dyv);
        object.y_velocity *= -1.0;
    }
    else if object.pos.1 + dyv > height / 2.0 {
        object.pos.1 = height - (object.pos.1 + dyv);
        object.y_velocity *= -1.0;
    }
    else {
        object.pos.1 += dyv;
    }
}

fn collision_check(player: &PhysObject, balls: &Vec<PhysObject>) -> Vec<f32> {
    let mut coll_balls = Vec::new();
    for ball in balls {
        let pdistance1 = ball.get_pos_p2() - player.get_pos_p2();
        if pdistance1.norm() < (player.bbox_size + ball.bbox_size) {
            coll_balls.push(ball.id)
        }
    }
    return coll_balls;
}

fn ball_pickup(player: &mut PhysObject, balls: &Vec<PhysObject>) {
    if player.hold != 0.0 {
        return; //already holding
    }
    let coll_balls = collision_check(player, balls);
    if !coll_balls.is_empty() {
        let ball = ball_id_to_elem(balls, coll_balls[0]).unwrap();
        player.hold = balls[ball].id;
    }
}

fn ball_drop(player: &mut PhysObject, balls: &mut Vec<PhysObject>) {
    let ball = ball_id_to_elem(balls, player.hold);
    match ball {
        Some(x) => {
            balls[x].x_velocity = player.x_velocity;
            balls[x].y_velocity = player.y_velocity;
        },
        _ => ()
    }
    player.hold = 0.0;
}

fn collision_check_score(player: &PhysObject, balls: &Vec<PhysObject>, alligment: f32) -> bool {
    for ball in balls {
        let pdistance = ball.get_pos_p2() - player.get_pos_p2();
        if pdistance.norm() < (player.bbox_size + ball.bbox_size) && ball.hold == alligment {
            return true;
        }
    }
    return false;
}

/// Translates the world coordinate system, which
/// has Y pointing up and the origin at the center,
/// to the screen coordinate system, which has Y
/// pointing downward and the origin at the top-left,
fn world_to_screen_coords(screen_width: f32, screen_height: f32, point: Point2) -> Point2 {
    let x = point.x + screen_width / 2.0;
    let y = screen_height - (point.y + screen_height / 2.0);
    Point2::new(x, y)
}

/// **********************************************************************
/// So that was the real meat of our game.  Now we just need a structure
/// to contain the images and font. That we need to hang on to; this
/// is our "asset management system".  All the file names and such are
/// just hard-coded.
/// **********************************************************************

struct Assets {
    player_red_image: graphics::Image,
    player_blue_image: graphics::Image,
    ball_image: graphics::Image,
    ball_red_image: graphics::Image,
    ball_blue_image: graphics::Image,
    font: graphics::Font,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_red_image = graphics::Image::new(ctx, "/player_red.png")?;
        let player_blue_image = graphics::Image::new(ctx, "/player_blue.png")?;
        let ball_image = graphics::Image::new(ctx, "/ball.png")?;
        let ball_red_image = graphics::Image::new(ctx, "/ball_red.png")?;
        let ball_blue_image = graphics::Image::new(ctx, "/ball_blue.png")?;
        let font = graphics::Font::new(ctx, "/CandyBeans.ttf")?;

        Ok(Assets {
            player_red_image,
            player_blue_image,
            ball_image,
            ball_red_image,
            ball_blue_image,
            font,
        })
    }

    fn actor_image(&mut self, object: &PhysObject) -> &mut graphics::Image {
        match object.tag {
            PhysType::Player => {
                match object.id {
                    x if x == 1.0 => &mut self.player_red_image,
                    x if x == 2.0 => &mut self.player_blue_image,
                    _ => &mut self.ball_image,
                }
            }
            PhysType::Ball => {
                match object.hold {
                    x if x == 1.0 => &mut self.ball_red_image,
                    x if x == 2.0 => &mut self.ball_blue_image,
                    _ => &mut self.ball_image,
                }
            },
        }
    }
}

/// **********************************************************************
/// The `InputState` is exactly what it sounds like, it just keeps track of
/// the user's input state so that we turn keyboard events into something
/// state-based and device-independent.
/// **********************************************************************
#[derive(Debug)]
pub struct InputState {
    pub xaxis1pos: f32,
    pub xaxis1neg: f32,
    pub yaxis1pos: f32,
    pub yaxis1neg: f32,
    pub holdball: bool,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            xaxis1pos: 0.0,
            xaxis1neg: 0.0,
            yaxis1pos: 0.0,
            yaxis1neg: 0.0,
            holdball: false,
        }
    }
}

/// **********************************************************************
/// Now we're getting into the actual game loop.  The `MainState` is our
/// game's "global" state, it keeps track of everything we need for
/// actually running the game.
/// **********************************************************************

    /// Mainstate

struct MainState {
    game: GameState,
    assets: Assets,
    source_player1: Option<String>,
    source_player2: Option<String>,
}

struct GameState {
    player1: PhysObject,
    player2: PhysObject,
    balls: Vec<PhysObject>,
    score1: i32,
    score2: i32,
    screen_width: f32,
    screen_height: f32,
    input1: InputState,
    input2: InputState,
}

impl MainState {
    fn new(ctx: &mut Context, source_player1: Option<String>, source_player2: Option<String>) -> GameResult<MainState> {
        println!("Game resource path: {:?}", ctx.filesystem);

        print_instructions();
        
        let (width, height) = graphics::drawable_size(ctx);

        let assets = Assets::new(ctx)?;
        let (player1, player2, balls) = reset_field(width);
        let g = GameState {
            player1,
            player2,
            balls,
            score1: 0,
            score2: 0,
            screen_width: width,
            screen_height: height,
            input1: InputState::default(),
            input2: InputState::default(),
        };
        let s = MainState {
            game: g,
            assets: assets,
            source_player1: source_player1,
            source_player2: source_player2,
        };

        Ok(s)
    }

    /* fn check_for_level_respawn(&mut self) {
        if self.game.score1 >= 3 || self.game.score2 >= 3 {
            // Reset game
        }
    } */
}

/// **********************************************************************
/// A couple of utility functions.
/// **********************************************************************

fn print_instructions() {
    println!();
    println!("Welcome to ASTROBLASTO 2: Electric Bogaloo!");
    println!();
    println!("How to play:");
    println!("Player 1: WASD to move your ship, space bar to pick up and release balls");
    println!("Player 2: arrow keys to move your ship, enter to pick up and release balls");
    println!();
}

fn draw_physobject(
    assets: &mut Assets,
    ctx: &mut Context,
    object: &PhysObject,
    world_coords: (f32, f32),
) -> GameResult {
    let (screen_w, screen_h) = world_coords;
    let pos = world_to_screen_coords(screen_w, screen_h, object.get_pos_p2());
    let image = assets.actor_image(object);
    let drawparams = graphics::DrawParam::new()
        .dest(pos)
        .offset(Point2::new(0.5, 0.5));
    graphics::draw(ctx, image, drawparams)
}

/// **********************************************************************
/// Now we implement the `EventHandler` trait from `ggez::event`, which provides
/// ggez with callbacks for updating and drawing our game, as well as
/// handling input events.
/// **********************************************************************
impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        const DESIRED_FPS: u32 = 60;

        while timer::check_update_time(ctx, DESIRED_FPS) {
            let seconds = 1.0 / (DESIRED_FPS as f32);

            // Update the player state based on the user input.
            match self.source_player1.as_ref() {
                Some(scriptname1) => {
                    self.game.input1 = ai_generate_input(&(self.game), &scriptname1, true);
                },
                None => ()
            }
            match self.source_player2.as_ref() {
                Some(scriptname2) => {
                    self.game.input2 = ai_generate_input(&(self.game), &scriptname2, false);
                },
                None => ()
            }
            player_handle_input(&mut self.game.player1, &self.game.input1, &mut self.game.balls);
            player_handle_input(&mut self.game.player2, &self.game.input2, &mut self.game.balls);

            // Update the physics for all PhysObjects.
            // First the players...
            update_object_position(&mut self.game.player1, -self.game.screen_width as f32 / 2.0, 0.0, self.game.screen_height as f32, seconds);
            update_object_position(&mut self.game.player2, 0.0, self.game.screen_width as f32 / 2.0, self.game.screen_height as f32, seconds);
            // Then the balls!
            for ball in &mut self.game.balls {
                update_object_position(ball, -self.game.screen_width as f32 / 2.0, self.game.screen_width as f32 / 2.0, self.game.screen_height as f32, seconds);
                ball_halt(ball, seconds)
            }

            ball_follow(&self.game.player1, &mut self.game.balls, 32.0);
            ball_follow(&self.game.player2, &mut self.game.balls, -32.0);

            //self.check_for_level_respawn();

            let (width, _) = graphics::drawable_size(ctx);
            if collision_check_score(&self.game.player1, &self.game.balls, 2.0) {
                self.game.score2 += 1;
                let (fresh_player1, fresh_player2, fresh_balls) = reset_field(width);
                self.game.player1 = fresh_player1;
                self.game.player2 = fresh_player2;
                self.game.balls = fresh_balls;
            }
            if collision_check_score(&self.game.player2, &self.game.balls, 1.0) {
                self.game.score1 += 1;
                let (fresh_player1, fresh_player2, fresh_balls) = reset_field(width);
                self.game.player1 = fresh_player1;
                self.game.player2 = fresh_player2;
                self.game.balls = fresh_balls;
            }
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        // Our drawing is quite simple.
        // Just clear the screen...
        graphics::clear(ctx, graphics::Color::new(0.2, 0.2, 0.2, 1.0));

        // Loop over all objects drawing them...
        {
            let assets = &mut self.assets;
            let coords = (self.game.screen_width, self.game.screen_height);

            let p1 = &self.game.player1;
            draw_physobject(assets, ctx, p1, coords)?;
            let p2 = &self.game.player2;
            draw_physobject(assets, ctx, p2, coords)?;

            for b in &self.game.balls {
                draw_physobject(assets, ctx, b, coords)?;
            }
        }

        // And draw the GUI elements in the right places.
        let score1_dest = Point2::new(10.0, 10.0);
        let score2_dest = Point2::new(480.0, 10.0);

        let score1_str = format!("Score: {}", self.game.score1);
        let score2_str = format!("Score: {}", self.game.score2);

        let score1_display = graphics::Text::new((score1_str, self.assets.font, 48.0));
        let score2_display = graphics::Text::new((score2_str, self.assets.font, 48.0));
        graphics::draw(ctx, &score1_display, (score1_dest, 0.0, graphics::Color::new(1.0, 0.3, 0.3, 1.0)))?;
        graphics::draw(ctx, &score2_display, (score2_dest, 0.0, graphics::Color::new(0.3, 0.3, 1.0, 1.0)))?;

        // Then we flip the screen...
        graphics::present(ctx)?;

        // And yield the timeslice
        // This tells the OS that we're done using the CPU but it should
        // get back to this program as soon as it can.
        // This ideally prevents the game from using 100% CPU all the time
        // even if vsync is off.
        // The actual behavior can be a little platform-specific.
        timer::yield_now();
        Ok(())
    }

    // TODO Keyboard events

    // Handle key events.  These just map keyboard events
    // and alter our input state appropriately.
    fn key_down_event(
        &mut self,
        ctx: &mut Context,
        keycode: KeyCode,
        _keymod: KeyMods,
        _repeat: bool,
    ) {
        match keycode {
            KeyCode::P => {
                let img = graphics::screenshot(ctx).expect("Could not take screenshot");
                img.encode(ctx, graphics::ImageFormat::Png, "/screenshot.png")
                .expect("Could not save screenshot");
            }
            KeyCode::Escape => event::quit(ctx),
            _ => (), // Do nothing
        }
        if self.source_player1.is_none() {
            match keycode {
                KeyCode::W => {
                    self.game.input1.yaxis1pos = 1.0;
                }
                KeyCode::S => {
                    self.game.input1.yaxis1neg = -1.0;
                }
                KeyCode::D => {
                    self.game.input1.xaxis1pos = 1.0;
                }
                KeyCode::A => {
                    self.game.input1.xaxis1neg = -1.0;
                }
                KeyCode::Space => {
                    self.game.input1.holdball = true;
                }
                _ => (), // Do nothing
            }
        }
        if self.source_player2.is_none() {
            match keycode {
                KeyCode::Up => {
                    self.game.input2.yaxis1pos = 1.0;
                }
                KeyCode::Down => {
                    self.game.input2.yaxis1neg = -1.0;
                }
                KeyCode::Right => {
                    self.game.input2.xaxis1pos = 1.0;
                }
                KeyCode::Left => {
                    self.game.input2.xaxis1neg = -1.0;
                }
                KeyCode::Return => {
                    self.game.input2.holdball = true;
                }
                _ => (), // Do nothing
            }
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
        if self.source_player1.is_none() {
            match keycode {
                KeyCode::W => {
                    self.game.input1.yaxis1pos = 0.0;
                }
                KeyCode::S => {
                    self.game.input1.yaxis1neg = 0.0;
                }
                KeyCode::D => {
                    self.game.input1.xaxis1pos = 0.0;
                }
                KeyCode::A => {
                    self.game.input1.xaxis1neg = 0.0;
                }
                KeyCode::Space => {
                    self.game.input1.holdball = false;
                }
                _ => (), // Do nothing
            }
        }
        if self.source_player2.is_none() {
            match keycode {
                KeyCode::Up => {
                    self.game.input2.yaxis1pos = 0.0;
                }
                KeyCode::Down => {
                    self.game.input2.yaxis1neg = 0.0;
                }
                KeyCode::Right => {
                    self.game.input2.xaxis1pos = 0.0;
                }
                KeyCode::Left => {
                    self.game.input2.xaxis1neg = 0.0;
                }
                KeyCode::Return => {
                    self.game.input2.holdball = false;
                }
                _ => (), // Do nothing
            }
        }
    }
}

//AI-scripting functions

fn test_plugin(a: isize, b: isize, name: &str) -> isize {
    if Library::new(name).is_err() {
        eprintln!("Error during add test: {:?}", Library::new(name).err());
        panic!();
    }

    let lib = Library::new(name).unwrap();
    unsafe {
        let func: Symbol<AddFunc> = lib.get(b"add").unwrap();
        let answer = func(a, b);
        answer
        
    }
}

fn ai_generate_input(state: &GameState, name: &str, p1: bool) -> InputState {
    let lib = Library::new(name).unwrap();

    unsafe {
        let func: Symbol<AIFunc> = lib.get(b"calculate_move").unwrap();
        func(state, p1)
    }
}

fn compile_file(path: &Path) {
    let mut compile_file = Command::new("rustc");
    compile_file.args(&["--crate-type", "cdylib", path.as_os_str().to_str().unwrap()]).status().expect("process failed to execute");
}

/// **********************************************************************
/// Finally our main function!  Which merely sets up a config and calls
/// `ggez::event::run()` with our `EventHandler` type. (Yeah right...)
/// **********************************************************************

pub fn main() -> GameResult {
    let args: Vec<String> = env::args().collect();
    //AI script loading
    let paths = fs::read_dir("src/script/").unwrap();

    //Name of selected script with functioning test function
    // let mut maybe_name: Option<String> = None;
    println!("Reading files from script folder:");

    //Iterate over paths
    let mut valid_scripts: Vec<String> = Vec::new();

    for path_prewrap in paths {
        let path = path_prewrap.unwrap().path();

        println!("PreFilter: {}", path.file_name().unwrap().to_str().unwrap());

        //Has to be
        //* Not directory
        //* Not the structs-file
        //* Extension is .rs
        if path.is_dir() {
            continue
        }
        if path.file_name().unwrap() == "structs.rs" {
            continue
        }
        if path.extension().unwrap() != "rs" {
            continue
        }

        //All pre-requirements met
        //Compile and test the script
        compile_file(path.as_path());
        println!("FileName: {}", path.file_name().unwrap().to_str().unwrap());
        println!("Name: {}", path.display());

        println!("Test upcoming:");

        //Tests the code, 1 + 3 = 4. Mostly to check connectivity

        let full_path;
        if cfg!(windows) {
            full_path = format!("{}/{}", env::current_dir().unwrap().to_string_lossy(), path.file_name().unwrap().to_str().unwrap().replace("rs","dll"));
        }
        else if cfg!(unix) {
            full_path = format!("{}/lib{}", env::current_dir().unwrap().to_string_lossy(), path.file_name().unwrap().to_str().unwrap().replace("rs","so"));
        }
        else {
            eprintln!("What the fuck?!");
            panic!();
        }
        println!("wee");
      
        let answer = test_plugin(1,3, full_path.as_str());
        println!("{}+{}:{}",1,3,answer);

        if answer != 4 {
            continue
            //Not correct, script disqualified
        }

        println!("Test finished:");

        //If it has not panicked by now, store the script file name
        valid_scripts.push(full_path);
        //maybe_name = Some(full_path);
    }

    // We add the CARGO_MANIFEST_DIR/resources to the resource paths
    // so that ggez will look in our cargo project directory for files.
    let resource_dir = if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push("resources");
        path
    } else {
        path::PathBuf::from("./resources")
    };

    let cb = ContextBuilder::new("astroblasto", "ggez")
        .window_setup(conf::WindowSetup::default().title("Astroblasto!"))
        .window_mode(conf::WindowMode::default().dimensions(640.0, 480.0))
        .add_resource_path(resource_dir);

    let (ctx, events_loop) = &mut cb.build()?;

    let mut player1: Option<String> = None;
    let mut player2: Option<String> = None;

    for valid in valid_scripts {
        //println!("currently checking: {} against {} and {}", valid, args.get(1).unwrap(), args.get(2).unwrap());
        match args.get(1) {
            Some(arg) => {
                if valid.contains(arg) {
                    player1 = Some(valid.clone());
                }
            },
            None => ()
        }
        match args.get(2) {
            Some(arg) => {
                if valid.contains(arg) {
                    player2 = Some(valid.clone());
                }
            },
            None => ()
        }
    }
    
    match player1.clone() {
        Some(name) => println!("Script {} loaded for P1", name),
        None => println!("No script loaded for P1"),
    }
    match player2.clone() {
        Some(name) => println!("Script {} loaded for P2", name),
        None => println!("No script loaded for P2"),
    }

    let game = &mut MainState::new(ctx, player1, player2)?;
    event::run(ctx, events_loop, game)
}