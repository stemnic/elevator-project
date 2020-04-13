use elevator_driver::*;
use network_rust::*;
use std::thread;
use std::sync::mpsc::*;

mod tasks;
mod elev_controller;

fn main() {
    let (network_sender, network_reciver) = channel::<elev_controller::ElevatorButtonEvent>();
    thread::spawn(move || {
        let socket = network_rust::bcast::BcastReceiver::new(elev_controller::BCAST_PORT).unwrap();
        
        thread::spawn(move || {
            socket.run(network_sender);
        });
    });
    let mut taskmanager = tasks::TaskManager::new().unwrap();
    
    loop {
        match network_reciver.try_recv() {
            Ok(data) => {
                match data.request {
                    elev_controller::RequestType::Request => {
                        taskmanager.add_new_task(elev_controller::Order {order_type: data.action, floor: data.floor}, data.origin);
                    }
                    elev_controller::RequestType::Taken => {
                        taskmanager.set_task_taken(elev_controller::Order {order_type: data.action, floor: data.floor}, data.origin);
                    }
                    elev_controller::RequestType::Complete => {
                        taskmanager.set_task_complete(elev_controller::Order {order_type: data.action, floor: data.floor}, data.origin);
                    }
                }
            }
            Err(_) => {}
        }
        taskmanager.run_task_state_machine();
    }
}
