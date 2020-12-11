pub struct GameState {
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

#[derive(Debug)]
pub struct PhysObject {
    tag: PhysType,
    id: f32,
    hold: f32,
    pos: (f32, f32),
    x_velocity: f32,
    y_velocity: f32,
    bbox_size: f32,
}

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

#[derive(Debug)]
enum PhysType {
    Player,
    Ball
}