use elevator_driver::*;
use network_rust::*;
use std::thread;
use std::sync::mpsc::*;

mod tasks;
mod elev_controller;

fn main() {
    let (network_sender, network_reciver) = channel::<elev_controller::ElevatorButtonEvent>();
    let (internal_sender, internal_reciver) = channel::<elev_controller::ElevatorButtonEvent>();
    thread::spawn(move || {
        let socket = network_rust::bcast::BcastReceiver::new(elev_controller::BCAST_PORT).unwrap();
        
        thread::spawn(move || {
            socket.run(network_sender);
        });
    });
    let mut taskmanager = tasks::TaskManager::new(internal_sender).unwrap();
    
    loop {
        match network_reciver.try_recv() {
            Ok(data) => {
                handle_network_message(&mut taskmanager, data);
            }
            Err(_) => {}
        }
        match internal_reciver.try_recv() {
            Ok(data) => {
                handle_network_message(&mut taskmanager, data);
            }
            Err(_) => {}
        }
        taskmanager.run_task_state_machine();
    }
}

fn handle_network_message(task_mgr: &mut tasks::TaskManager, msg: elev_controller::ElevatorButtonEvent) {
    match msg.request {
        elev_controller::RequestType::Request => {
            task_mgr.add_new_task(elev_controller::Order {order_type: msg.action, floor: msg.floor}, msg.origin);
        }
        elev_controller::RequestType::Taken => {
            task_mgr.set_task_taken(elev_controller::Order {order_type: msg.action, floor: msg.floor}, msg.origin);
        }
        elev_controller::RequestType::Complete => {
            task_mgr.set_task_complete(elev_controller::Order {order_type: msg.action, floor: msg.floor}, msg.origin);
        }
    }
}