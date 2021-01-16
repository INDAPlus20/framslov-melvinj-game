
//! An Asteroids-ish example game to show off ggez.
//! The idea is that this game is simple but still
//! non-trivial enough to be interesting.

use ggez;
use ggez::audio;
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
type AIFunc = unsafe fn(&GameState) -> InputState;

type Point2 = na::Point2<f32>;

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

/// *********************************************************************
/// Now we have some constructor functions for different game objects.
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
    balls.append(&mut create_balls_collumn((balls_num / 2.0).floor(), distance + 50.0));

    return balls;
}

fn create_balls_collumn(balls_num: f32, distance: f32) -> Vec<PhysObject> {
    let space = 50.0;
    let mut space_iter = -((balls_num - 1.0) * 50.0) / 2.0;
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

/// Acceleration in pixels per second squared.
const PLAYER_ACCELERATION: f32 = 8.0;
/// Max velocity in pixels per second
const MAX_PHYSICS_VEL: f32 = 200.0;
/// Deacceleration in pixels per second squared.
const BALL_DRAG: f32 = 20.0;

fn player_handle_input(object: &mut PhysObject, input: &InputState) {
    object.x_velocity += PLAYER_ACCELERATION * (input.xaxis1pos + input.xaxis1neg);
    object.y_velocity += PLAYER_ACCELERATION * (input.yaxis1pos + input.yaxis1neg);
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
/// to contain the images, sounds, etc. that we need to hang on to; this
/// is our "asset management system".  All the file names and such are
/// just hard-coded.
/// **********************************************************************

    // TODO Handle assets

struct Assets {
    player_image: graphics::Image,
    shot_image: graphics::Image,
    ball_image: graphics::Image,
    ball_red_image: graphics::Image,
    font: graphics::Font,
    shot_sound: audio::Source,
    hit_sound: audio::Source,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        let player_image = graphics::Image::new(ctx, "/player.png")?;
        let shot_image = graphics::Image::new(ctx, "/shot.png")?;
        let ball_image = graphics::Image::new(ctx, "/ball.png")?;
        let ball_red_image = graphics::Image::new(ctx, "/ball_red.png")?;
        let font = graphics::Font::new(ctx, "/DejaVuSerif.ttf")?;

        let shot_sound = audio::Source::new(ctx, "/pew.ogg")?;
        let hit_sound = audio::Source::new(ctx, "/boom.ogg")?;

        Ok(Assets {
            player_image,
            shot_image,
            ball_image,
            ball_red_image,
            font,
            shot_sound,
            hit_sound,
        })
    }

    fn actor_image(&mut self, object: &PhysObject) -> &mut graphics::Image {
        match object.tag {
            PhysType::Player => &mut self.player_image,
            PhysType::Ball => {
                match object.hold {
                    x if x == 1.0 => &mut self.ball_red_image,
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
    scriptname: String,
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
            scriptname: name,
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
            let scriptname = &(&(self.scriptname)).clone();
            println!("Attempting to generate input");
            self.game.input2 = ai_generate_input(&mut self.game, scriptname);
            player_handle_input(&mut self.game.player1, &self.game.input1);
            
            /*self.player_shot_timeout -= seconds;
            if self.input.fire && self.player_shot_timeout < 0.0 {
                self.fire_player_shot();
            }*/

            // Update the physics for all actors.
            // First the players...
            update_object_position(&mut self.game.player1, -self.game.screen_width as f32 / 2.0, 0.0, self.game.screen_height as f32, seconds);
            update_object_position(&mut self.game.player2, 0.0, self.game.screen_width as f32 / 2.0, self.game.screen_height as f32, seconds);
            // Then the balls...
            for ball in &mut self.game.balls {
                update_object_position(ball, -self.game.screen_width as f32 / 2.0, self.game.screen_width as f32 / 2.0, self.game.screen_height as f32, seconds);
                ball_halt(ball, seconds)
            }

            ball_follow(&self.game.player1, &mut self.game.balls);
            ball_follow(&self.game.player2, &mut self.game.balls);

            // Handle the results of things moving:
            // collision detection, object death, and if
            // we have killed all the rocks in the level,
            // spawn more of them.

            self.check_for_level_respawn();

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
        let score2_dest = Point2::new(250.0, 10.0);

        let score1_str = format!("Player 1: {}", self.game.score1);
        let score2_str = format!("Player 2: {}", self.game.score2);

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
            KeyCode::P => {
                let img = graphics::screenshot(ctx).expect("Could not take screenshot");
                img.encode(ctx, graphics::ImageFormat::Png, "/screenshot.png")
                    .expect("Could not save screenshot");
            }
            KeyCode::Escape => event::quit(ctx),
            _ => (), // Do nothing
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
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
                            self.game.balls[x].hold = 1.0;
                        }
                        _ => ()
                    }
                }
                self.game.player1.hold = 0.0;
            }
            _ => (), // Do nothing
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

fn ai_generate_input(state: &mut GameState, name: &str) -> InputState {
    let lib = Library::new(name).unwrap();
    unsafe {
        let func: Symbol<AIFunc> = lib.get(b"calculate_move").unwrap();
        func(state)
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
        println!("{}+{}:{}",1,3,test_plugin(1,3, full_path.as_str()));

        println!("Test finished:");

        //If it has not panicked by now, store the script file name
        maybe_name = Some(full_path);
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