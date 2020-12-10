
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
    pos: Point2,
    x_velocity: f32,
    y_velocity: f32,
    bbox_size: f32,
}

thread_local!(static BALL_ID: Cell<f32> = Cell::new(2.0));

impl PhysObject {
    fn new_ball_id(pos: Point2,) -> PhysObject {
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
        pos: Point2::origin(),
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
    balls.push(PhysObject::new_ball_id(Point2::new(-x, y)));
    balls.push(PhysObject::new_ball_id(Point2::new(x, y)));
    
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

    if object.pos.x + dxv < -width / 2.0 {
        object.pos.x = -width - (object.pos.x + dxv);
        object.x_velocity *= -1.0;
    }
    else if object.pos.x + dxv > width / 2.0 {
        object.pos.x = width - (object.pos.x + dxv);
        object.x_velocity *= -1.0;
    }
    else {
        object.pos.x += dxv;
    }

    if object.pos.y + dyv < height / -2.0 {
        object.pos.y = -height - (object.pos.y + dyv);
        object.y_velocity *= -1.0;
    }
    else if object.pos.y + dyv > height / 2.0 {
        object.pos.y = height - (object.pos.y + dyv);
        object.y_velocity *= -1.0;
    }
    else {
        object.pos.y += dyv;
    }
}

/// Takes an actor and wraps its position to the bounds of the
/// screen, so if it goes off the left side of the screen it
/// will re-enter on the right side and so on.
fn wrap_actor_position(object: &mut PhysObject, sx: f32, sy: f32) {
    // Wrap screen
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;
    if object.pos.x > screen_x_bounds {
        object.pos.x -= sx;
    } else if object.pos.x < -screen_x_bounds {
        object.pos.x += sx;
    };
    if object.pos.y > screen_y_bounds {
        object.pos.y -= sy;
    } else if object.pos.y < -screen_y_bounds {
        object.pos.y += sy;
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
struct InputState {
    xaxis1pos: f32,
    xaxis1neg: f32,
    yaxis1pos: f32,
    yaxis1neg: f32,
    xaxis2pos: f32,
    xaxis2neg: f32,
    yaxis2pos: f32,
    yaxis2neg: f32,
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
    player1: PhysObject,
    player2: PhysObject,
    balls: Vec<PhysObject>,
    score1: i32,
    score2: i32,
    assets: Assets,
    screen_width: f32,
    screen_height: f32,
    input: InputState,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        println!("Game resource path: {:?}", ctx.filesystem);

        print_instructions();
        
        let (width, height) = graphics::drawable_size(ctx);

        let assets = Assets::new(ctx)?;
        let player1 = create_player();
        let player2 = create_player();
        let balls = create_balls(6.0, (width, height));

        let s = MainState {
            player1,
            player2,
            balls,
            score1: 0,
            score2: 0,
            assets,
            screen_width: width,
            screen_height: height,
            input: InputState::default(),
        };

        Ok(s)
    }

    fn collision_check(&mut self) -> (Vec<f32>, Vec<f32>) {
        let mut balls1 = Vec::new();
        let mut balls2 = Vec::new();
        for ball in &mut self.balls {
            let pdistance1 = ball.pos - self.player1.pos;
            if pdistance1.norm() < (self.player1.bbox_size + ball.bbox_size) {
                balls1.push(ball.id)
            }
            let pdistance2 = ball.pos - self.player2.pos;
            if pdistance2.norm() < (self.player2.bbox_size + ball.bbox_size) {
                balls2.push(ball.id)
            }
        }
        return (balls1, balls2)
    }

    fn check_for_level_respawn(&mut self) {
        if self.score1 >= 3 || self.score2 >= 3 {
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
    let pos = world_to_screen_coords(screen_w, screen_h, object.pos);
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
            player_handle_input(&mut self.player1, &self.input);
            
            /*self.player_shot_timeout -= seconds;
            if self.input.fire && self.player_shot_timeout < 0.0 {
                self.fire_player_shot();
            }*/

            // Update the physics for all actors.
            // First the player...
            update_object_position(&mut self.player1, self.screen_width as f32, self.screen_height as f32, seconds);
            wrap_actor_position(&mut self.player1, self.screen_width as f32, self.screen_height as f32);

            // Then the balls...
            for object in &mut self.balls {
                update_object_position(object, self.screen_width as f32, self.screen_height as f32, seconds);
                //wrap_actor_position(object, self.screen_width as f32, self.screen_height as f32);
                //handle_timed_life(object, seconds);
            }

            ball_follow(&self.player1, &mut self.balls);
            ball_follow(&self.player2, &mut self.balls);

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
            let coords = (self.screen_width, self.screen_height);

            let p = &self.player1;
            draw_physobject(assets, ctx, p, coords)?;

            for b in &self.balls {
                draw_physobject(assets, ctx, b, coords)?;
            }
        }

        // And draw the GUI elements in the right places.
        let score1_dest = Point2::new(10.0, 10.0);
        let score2_dest = Point2::new(250.0, 10.0);

        let score1_str = format!("X Velocity: {}", self.player1.x_velocity);
        let score2_str = format!("Y Velocity: {}", self.player1.y_velocity);

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
                self.input.yaxis1pos = 1.0;
            }
            KeyCode::S => {
                self.input.yaxis1neg = -1.0;
            }
            KeyCode::D => {
                self.input.xaxis1pos = 1.0;
            }
            KeyCode::A => {
                self.input.xaxis1neg = -1.0;
            }
            KeyCode::Space => {
                let coll_balls = self.collision_check();
                if self.player1.hold == 0.0 && !coll_balls.0.is_empty() {
                    self.player1.hold = coll_balls.0[0];
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
                self.input.yaxis1pos = 0.0;
            }
            KeyCode::S => {
                self.input.yaxis1neg = 0.0;
            }
            KeyCode::D => {
                self.input.xaxis1pos = 0.0;
            }
            KeyCode::A => {
                self.input.xaxis1neg = 0.0;
            }
            KeyCode::Space => {
                let id = ball_id_to_elem(&self.balls, self.player1.hold);
                if id.is_some() {
                    match id {
                        Some(x) => {self.balls[x].x_velocity = self.player1.x_velocity;
                            self.balls[x].y_velocity = self.player1.y_velocity;}
                        _ => ()
                    }
                }
                self.player1.hold = 0.0;
            }
            _ => (), // Do nothing
        }
    }
}

/// **********************************************************************
/// Finally our main function!  Which merely sets up a config and calls
/// `ggez::event::run()` with our `EventHandler` type.
/// **********************************************************************

pub fn main() -> GameResult {
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

    let game = &mut MainState::new(ctx)?;
    event::run(ctx, events_loop, game)
}