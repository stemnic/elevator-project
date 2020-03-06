use elevator_driver::*;
use network_rust::*;

mod tasks;
mod elev_controller;

fn main() {
    println!("Hello, world!");
    let mut controller = elev_controller::Elev_Controller::new().unwrap();
    controller.add_order(elev_controller::Order{floor: 3});
    loop {
        controller.handle_order();
    }
}
