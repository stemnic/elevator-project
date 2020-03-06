use std::io;
use elevator_driver::*;

struct Task {
    id: usize, // Necessery?
    elegebility: usize, // Cost function
    taken: bool,
    complete: bool,
}

impl Task {
    /*
    pub fn new() -> io::Result<Self> {
        let task = Task {};

        Ok(&self)
    }
    */
}

const NUM_FLOORS: usize = 4;

enum Direction {
    UP,
    Down,
}

fn cost_function() {
    let direction = elevator_driver::elev_driver::ElevIo::new();
}

