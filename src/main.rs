use elevator_driver::*;
use network_rust::*;
use std::thread;
use std::sync::mpsc::*;

mod tasks;
mod elev_controller;

fn main() {
    println!("Starting");
    let mut controller = elev_controller::Elev_Controller::new().unwrap();
    println!("Done init!");
    controller.add_order(elev_controller::Order{floor: 3});
    controller.add_order(elev_controller::Order{floor: 0});
    controller.add_order(elev_controller::Order{floor: 2});
    let (sender, reciver) = channel::<elev_controller::ElevatorButtonEvent>();
    thread::spawn(move || {
        let socket = network_rust::bcast::BcastReceiver::new(elev_controller::BCAST_PORT).unwrap();
        
        thread::spawn(move || {
            socket.run(sender);
        });
    });
    loop {
        controller.handle_order();
        controller.check_buttons();
        match reciver.try_recv() {
            Ok(value) => {
                let new_order = elev_controller::Order{floor: value.floor};
                controller.add_order(new_order);
            }
            Err(_) => {}
        }
    }
}
