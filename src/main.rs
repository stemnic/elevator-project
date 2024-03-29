use elevator_driver::*;
use std::thread;
use std::sync::mpsc::*;
use std::env;
use regex::Regex;

mod task_manager;
mod elev_controller;

fn main() {
    println!("Starting Elevator server for {} floors", elev_driver::N_FLOORS);
    let args: Vec<String> = env::args().collect();
    let re = Regex::new(r"((?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)))+$").unwrap();
    let ip_addr = network_rust::localip::get_localip().unwrap().to_string();
    let lower_ip_part = re.find(&ip_addr).unwrap().as_str();

    let mut id = lower_ip_part.parse::<u32>().unwrap();
    let mut udp_broadcast_port = elev_controller::BCAST_PORT;
    let mut elevator_ip = elev_driver::DEFAULT_IP_ADDRESS;
    let mut elevator_port = elev_driver::DEFAULT_PORT;
    match args.len() {
        1 => {}
        2 => {
            let cmd = &args[1];
            if cmd.contains("--help") {
                println!("elevator-project (elevator id) (udp_broadcast_port) (elevator hardware ip) (elevator hardware port)");
                std::process::exit(0);
            }
            id = cmd.parse::<u32>().unwrap();
        }
        3 => {
            id = (&args[1]).parse::<u32>().unwrap();
            udp_broadcast_port = (&args[2]).parse::<u16>().unwrap();
        }
        4 => {
            id = (&args[1]).parse::<u32>().unwrap();
            udp_broadcast_port = (&args[2]).parse::<u16>().unwrap();
            elevator_ip = &args[3];
        }
        5 => {
            id = (&args[1]).parse::<u32>().unwrap();
            udp_broadcast_port = (&args[2]).parse::<u16>().unwrap();
            elevator_ip = &args[3];
            elevator_port = (&args[4]).parse::<u16>().unwrap();
        }
        _ => {
            println!("Invalid number of arguments!");
            println!("elevator-project (elevator id) (udp_broadcast_port) (elevator hardware ip) (elevator hardware port)");
            std::process::exit(0);
        }
    }
  
    let (network_sender, network_reciver) = channel::<elev_controller::ButtonEvent>();
    let (internal_sender, internal_reciver) = channel::<elev_controller::ButtonEvent>();
    thread::spawn(move || {
        let socket = network_rust::bcast::BcastReceiver::new(udp_broadcast_port).unwrap();
        socket.run(network_sender);
    });
    let mut taskmanager = task_manager::TaskManager::new(internal_sender, id, udp_broadcast_port, elevator_ip, elevator_port).unwrap();
    
    loop {
        loop {
            match network_reciver.try_recv() {
                Ok(data) => {
                    handle_network_message(&mut taskmanager, data);
                }
                Err(_) => {
                    break
                }
            }
        }
        loop {
            match internal_reciver.try_recv() {
                Ok(data) => {
                    handle_network_message(&mut taskmanager, data);
                }
                Err(_) => {
                    break
                }
            }
        }
        taskmanager.run_state_machine();
    }
}

fn handle_network_message(task_mgr: &mut task_manager::TaskManager, msg: elev_controller::ButtonEvent) {
    match msg.request {
        elev_controller::RequestType::Request => {
            task_mgr.add_new_task(msg.order, msg.origin);
        }
        elev_controller::RequestType::Taken => {
            task_mgr.set_task_taken(msg.order, msg.origin);
        }
        elev_controller::RequestType::Complete => {
            task_mgr.set_task_complete(msg.order, msg.origin);
        }
    }
}