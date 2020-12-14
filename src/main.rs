
//! An Asteroids-ish example game to show off ggez.
//! The idea is that this game is simple but still
//! non-trivial enough to be interesting.

use ggez;
use ggez::audio;
use ggez::audio::SoundSource;
use ggez::conf;
use ggez::event::{self, EventHandler, KeyCode, KeyMods};
use ggez::graphics;
use ggez::nalgebra as na;
use ggez::timer;
use ggez::{Context, ContextBuilder, GameResult};
use rand;

use std::env;
use std::path;
use std::cell::Cell;

use std::fs::File;
use std::io::prelude::*;
use std::process::Command;
use libloading::{Library, Symbol};
use std::fs::{self};
use std::path::Path;

type AddFunc = unsafe fn(isize, isize) -> isize;
type AIFunc = unsafe fn(&GameState) -> InputState;

type Point2 = na::Point2<f32>;
type Vector2 = na::Vector2<f32>;

/// *********************************************************************
/// Basic stuff, make some helpers for vector functions.
/// We use the nalgebra math library to provide lots of
/// math stuff.  This just adds some helpers.
/// **********************************************************************

/// Create a unit vector representing the
/// given angle (in radians)
fn vec_from_angle(angle: f32) -> Vector2 {
    let vx = angle.sin();
    let vy = angle.cos();
    Vector2::new(vx, vy)
}

/// Makes a random `Vector2` with the given max magnitude.
fn random_vec(max_magnitude: f32) -> Vector2 {
    let angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;
    let mag = rand::random::<f32>() * max_magnitude;
    vec_from_angle(angle) * (mag)
}

/// *********************************************************************
/// Now we define our Actors.
/// An Actor is anything in the game world.
/// We're not *quite* making a real entity-component system but it's
/// pretty close.  For a more complicated game you would want a
/// real ECS, but for this it's enough to say that all our game objects
/// contain pretty much the same data.
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

const PLAYER_BBOX: f32 = 12.0;
const ROCK_BBOX: f32 = 12.0;
const SHOT_BBOX: f32 = 6.0;

/// *********************************************************************
/// Now we have some constructor functions for different game objects.
/// **********************************************************************

fn create_player() -> PhysObject {
    PhysObject {
        tag: PhysType::Player,
        id: 1.0,
        hold: 0.0,
        pos: (0.0,0.0),//Origin
        x_velocity: 0.0,
        y_velocity: 0.0,
        bbox_size: PLAYER_BBOX
    }
}

fn create_balls(balls_num: f32, size: (f32, f32)) -> Vec<PhysObject> {
    let mut balls = Vec::new();
    let distance = 100.0;
    balls.append(&mut create_balls_collumn((balls_num / 2.0).ceil(), distance, size));
    balls.append(&mut create_balls_collumn((balls_num / 2.0).floor(), distance + 50.0, size));

    return balls;
}

fn create_balls_collumn(balls_num: f32, distance: f32, size: (f32, f32)) -> Vec<PhysObject> {
    let space = 50.0;
    let mut space_iter = -((balls_num - 1.0) * 50.0) / 2.0;
    let mut balls = Vec::new();
    for _ in 0..balls_num as i32 {
        balls.append(&mut create_ball_pair(distance, space_iter, size));
        space_iter += space;
    }

    return balls;
}

fn create_ball_pair(x: f32, y: f32, size: (f32, f32)) -> Vec<PhysObject> {
    let (width, height) = size;
    let mut balls = Vec::new();
    balls.push(PhysObject::new_ball_id((-x, y)));
    balls.push(PhysObject::new_ball_id((x, y)));
    
    return balls;
}

fn ball_id_to_elem(balls: &Vec<PhysObject>, id: f32) -> Option<usize> {
    for (index, ball) in balls.iter().enumerate() {
        if ball.id == id {
            return Some(index);
        }
    }
    
    return None;
}

fn ball_follow(player: &PhysObject, balls: &mut Vec<PhysObject>) {
    let index = ball_id_to_elem(&balls, player.hold);
    if index.is_some() {
        match index {
            Some(x) => balls[x].pos = player.pos,
            _ => ()
        }
    }
}

/// *********************************************************************
/// Now we make functions to handle physics.  We do simple Newtonian
/// physics (so we do have inertia), and cap the max speed so that we
/// don't have to worry too much about small objects clipping through
/// each other.
///
/// Our unit of world space is simply pixels, though we do transform
/// the coordinate system so that +y is up and -y is down.
/// **********************************************************************

/// How fast shots move.
const SHOT_SPEED: f32 = 200.0;
/// Angular velocity of how fast shots rotate.
const SHOT_ANG_VEL: f32 = 0.1;

/// Acceleration in pixels per second.
const PLAYER_THRUST: f32 = 100.0;
/// Rotation in radians per second.
const PLAYER_SPEED: f32 = 8.0;
/// Refire delay between shots, in seconds.
const PLAYER_SHOT_TIME: f32 = 0.5;
/// Max velocity in pixels per second
const MAX_PHYSICS_VEL: f32 = 200.0;

    // TODO 2D input based on player ID

fn player_handle_input(object: &mut PhysObject, input: &InputState) {
    object.x_velocity += PLAYER_SPEED * (input.xaxis1pos + input.xaxis1neg);
    object.y_velocity += PLAYER_SPEED * (input.yaxis1pos + input.yaxis1neg);
}

    // TODO Update position

fn update_object_position(object: &mut PhysObject, width: f32, height: f32, dt: f32) {
    // Clamp the velocity to the max *efficiently*
    if object.x_velocity.abs() > MAX_PHYSICS_VEL {
        object.x_velocity = object.x_velocity.signum() * MAX_PHYSICS_VEL;
    }
    if object.y_velocity.abs() > MAX_PHYSICS_VEL {
        object.y_velocity = object.y_velocity.signum() * MAX_PHYSICS_VEL;
    }

    let dxv = object.x_velocity * dt;
    let dyv = object.y_velocity * dt;

    if object.pos.0 + dxv < -width / 2.0 {
        object.pos.0 = -width - (object.pos.0 + dxv);
        object.x_velocity *= -1.0;
    }
    else if object.pos.0 + dxv > width / 2.0 {
        object.pos.0 = width - (object.pos.0 + dxv);
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

/// Takes an actor and wraps its position to the bounds of the
/// screen, so if it goes off the left side of the screen it
/// will re-enter on the right side and so on.
fn wrap_actor_position(object: &mut PhysObject, sx: f32, sy: f32) {
    // Wrap screen
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;
    if object.pos.0 > screen_x_bounds {
        object.pos.0 -= sx;
    } else if object.pos.0 < -screen_x_bounds {
        object.pos.0 += sx;
    };
    if object.pos.1 > screen_y_bounds {
        object.pos.1 -= sy;
    } else if object.pos.1 < -screen_y_bounds {
        object.pos.1 += sy;
    }
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
/// to contain the images, sounds, etc. that we need to hang on to; this
/// is our "asset management system".  All the file names and such are
/// just hard-coded.
/// **********************************************************************

    // TODO Handle assets

struct Assets {
    player_image: graphics::Image,
    shot_image: graphics::Image,
    rock_image: graphics::Image,
    font: graphics::Font,
    shot_sound: audio::Source,
    hit_sound: audio::Source,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_image = graphics::Image::new(ctx, "/player.png")?;
        let shot_image = graphics::Image::new(ctx, "/shot.png")?;
        let rock_image = graphics::Image::new(ctx, "/rock.png")?;
        let font = graphics::Font::new(ctx, "/DejaVuSerif.ttf")?;

        let shot_sound = audio::Source::new(ctx, "/pew.ogg")?;
        let hit_sound = audio::Source::new(ctx, "/boom.ogg")?;

        Ok(Assets {
            player_image,
            shot_image,
            rock_image,
            font,
            shot_sound,
            hit_sound,
        })
    }

    fn actor_image(&mut self, object: &PhysObject) -> &mut graphics::Image {
        match object.tag {
            PhysType::Player => &mut self.player_image,
            PhysType::Ball => &mut self.rock_image,
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
    pub xaxis2pos: f32,
    pub xaxis2neg: f32,
    pub yaxis2pos: f32,
    pub yaxis2neg: f32,
}

impl Default for InputState {
    fn default() -> Self {
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
    fn new(ctx: &mut Context, name: String) -> GameResult<MainState> {
        println!("Game resource path: {:?}", ctx.filesystem);

        print_instructions();
        
        let (width, height) = graphics::drawable_size(ctx);

        let assets = Assets::new(ctx)?;
        let player1 = create_player();
        let player2 = create_player();
        let balls = create_balls(6.0, (width, height));
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
            source_player1: None,
            source_player2: Some(name),
        };

        Ok(s)
    }

    fn collision_check(&mut self) -> (Vec<f32>, Vec<f32>) {
        let mut balls1 = Vec::new();
        let mut balls2 = Vec::new();
        for ball in &mut self.game.balls {
            let pdistance1 = ball.get_pos_p2() - self.game.player1.get_pos_p2();
            if pdistance1.norm() < (self.game.player1.bbox_size + ball.bbox_size) {
                balls1.push(ball.id)
            }
            let pdistance2 = ball.get_pos_p2() - self.game.player2.get_pos_p2();
            if pdistance2.norm() < (self.game.player2.bbox_size + ball.bbox_size) {
                balls2.push(ball.id)
            }
        }
        return (balls1, balls2)
    }

    fn check_for_level_respawn(&mut self) {
        if self.game.score1 >= 3 || self.game.score2 >= 3 {
            // Reset game
        }
    }
}

/// **********************************************************************
/// A couple of utility functions.
/// **********************************************************************

fn print_instructions() {
    println!();
    println!("Welcome to ASTROBLASTO!");
    println!();
    println!("How to play:");
    println!("L/R arrow keys rotate your ship, up thrusts, space bar fires");
    println!();
}

    // TODO Draw

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
                    self.game.input1 = ai_generate_input(&(self.game), &scriptname1);
                },
                None => ()
            }
            match self.source_player2.as_ref() {
                Some(scriptname2) => {
                    self.game.input2 = ai_generate_input(&(self.game), &scriptname2);
                },
                None => ()
            }
            player_handle_input(&mut self.game.player1, &self.game.input1);
            player_handle_input(&mut self.game.player2, &self.game.input2);
            
            /*self.player_shot_timeout -= seconds;
            if self.input.fire && self.player_shot_timeout < 0.0 {
                self.fire_player_shot();
            }*/

            // Update the physics for all actors.
            // First the player...
            update_object_position(&mut self.game.player1, self.game.screen_width as f32, self.game.screen_height as f32, seconds);
            wrap_actor_position(&mut self.game.player1, self.game.screen_width as f32, self.game.screen_height as f32);

            // Then the balls...
            for object in &mut self.game.balls {
                update_object_position(object, self.game.screen_width as f32, self.game.screen_height as f32, seconds);
                //wrap_actor_position(object, self.screen_width as f32, self.screen_height as f32);
                //handle_timed_life(object, seconds);
            }

            ball_follow(&self.game.player1, &mut self.game.balls);
            ball_follow(&self.game.player2, &mut self.game.balls);

            // Handle the results of things moving:
            // collision detection, object death, and if
            // we have killed all the rocks in the level,
            // spawn more of them.

            self.check_for_level_respawn();

            // Finally we check for our end state.
            // I want to have a nice death screen eventually,
            // but for now we just quit.
            
            /*if self.player.life <= 0.0 {
                println!("Game over!");
                let _ = event::quit(ctx);
            }*/
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        // Our drawing is quite simple.
        // Just clear the screen...
        graphics::clear(ctx, graphics::BLACK);

        // Loop over all objects drawing them...
        {
            let assets = &mut self.assets;
            let coords = (self.game.screen_width, self.game.screen_height);

            let p = &self.game.player1;
            draw_physobject(assets, ctx, p, coords)?;

            for b in &self.game.balls {
                draw_physobject(assets, ctx, b, coords)?;
            }
        }

        // And draw the GUI elements in the right places.
        let score1_dest = Point2::new(10.0, 10.0);
        let score2_dest = Point2::new(250.0, 10.0);

        let score1_str = format!("X Velocity: {}", self.game.player1.x_velocity);
        let score2_str = format!("Y Velocity: {}", self.game.player1.y_velocity);

        let score1_display = graphics::Text::new((score1_str, self.assets.font, 32.0));
        let score2_display = graphics::Text::new((score2_str, self.assets.font, 32.0));
        graphics::draw(ctx, &score1_display, (score1_dest, 0.0, graphics::WHITE))?;
        graphics::draw(ctx, &score2_display, (score2_dest, 0.0, graphics::WHITE))?;

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
                    let coll_balls = self.collision_check();
                    if self.game.player1.hold == 0.0 && !coll_balls.0.is_empty() {
                        self.game.player1.hold = coll_balls.0[0];
                    }
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
                    let coll_balls = self.collision_check();
                    if self.game.player2.hold == 0.0 && !coll_balls.1.is_empty() {
                        self.game.player2.hold = coll_balls.1[0];
                    }
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
                    let id = ball_id_to_elem(&self.game.balls, self.game.player1.hold);
                    if id.is_some() {
                        match id {
                            Some(x) => {
                                self.game.balls[x].x_velocity = self.game.player1.x_velocity;
                                self.game.balls[x].y_velocity = self.game.player1.y_velocity;
                            },
                            _ => ()
                        }
                    }
                    self.game.player1.hold = 0.0;
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
                    let id = ball_id_to_elem(&self.game.balls, self.game.player2.hold);
                    if id.is_some() {
                        match id {
                            Some(x) => {
                                self.game.balls[x].x_velocity = self.game.player2.x_velocity;
                                self.game.balls[x].y_velocity = self.game.player2.y_velocity;
                            },
                            _ => ()
                        }
                    }
                    self.game.player2.hold = 0.0;
                }
                _ => (), // Do nothing
            }
        }

/*
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
                let id = ball_id_to_elem(&self.game.balls, self.game.player1.hold);
                if id.is_some() {
                    match id {
                        Some(x) => {self.game.balls[x].x_velocity = self.game.player1.x_velocity;
                            self.game.balls[x].y_velocity = self.game.player1.y_velocity;}
                        _ => ()
                    }
                }
                self.game.player1.hold = 0.0;
            }
            _ => (), // Do nothing
        }*/
    }
}

//AI-scripting functions

fn test_plugin(a: isize, b: isize, name: &str) -> isize {
    if Library::new(name.replace("rs","dll")).is_err() {
        eprintln!("Error during add test: {:?}", Library::new(name.replace("rs","dll")).err());
        panic!();
    }

    let lib = Library::new(name.replace("rs","dll")).unwrap();
    unsafe {
        let func: Symbol<AddFunc> = lib.get(b"add").unwrap();
        let answer = func(a, b);
        answer
        
    }
    
}

fn ai_generate_input(state: &GameState, name: &str) -> InputState {
    let lib = Library::new(name.replace("rs","dll")).unwrap();
    unsafe {
        let func: Symbol<AIFunc> = lib.get(b"calculate_move").unwrap();
        func(state)
    }
}

fn compile_file(path: &Path) {
    let mut compile_file = Command::new("cmd");
    compile_file.args(&["/C", "rustc", "--crate-type", "cdylib", path.as_os_str().to_str().unwrap()]).status().expect("process failed to execute");
}

/// **********************************************************************
/// Finally our main function!  Which merely sets up a config and calls
/// `ggez::event::run()` with our `EventHandler` type.
/// **********************************************************************

pub fn main() -> GameResult {

    //AI script loading
    let paths = fs::read_dir("src/script/").unwrap();

    //Name of selected script with functioning test function
    let mut maybe_name: Option<String> = None;
    println!("Reading files from script folder:");

    //Iterate over paths
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
        let temp = path.file_name().unwrap().to_str().unwrap();
        println!("wee");
        println!("{}+{}:{}",1,3,test_plugin(1,3, temp));

        println!("Test finished:");

        //If it has not panicked by now, store the script file name
        maybe_name = Some(path.file_name().unwrap().to_os_string().into_string().unwrap());
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

    
    if maybe_name.is_none() {
        eprintln!("No script"); 
        panic!("NO SCRIPT LOADED")
    }

    let game = &mut MainState::new(ctx, maybe_name.unwrap())?;
    event::run(ctx, events_loop, game)
}