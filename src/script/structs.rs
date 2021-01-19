pub struct GameState {
    pub player1: PhysObject,
    pub player2: PhysObject,
    pub balls: Vec<PhysObject>,
    pub score1: i32,
    pub score2: i32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub input1: InputState,
    pub input2: InputState,
}

#[derive(Debug)]
pub struct PhysObject {
    pub tag: PhysType,
    pub id: f32,
    pub hold: f32,
    pub pos: (f32, f32),
    pub x_velocity: f32,
    pub y_velocity: f32,
    pub bbox_size: f32,
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
    pub holdball: bool,
}

#[derive(Debug)]
enum PhysType {
    Player,
    Ball
}