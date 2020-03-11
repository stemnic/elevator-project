use elevator_driver::elev_driver::*;
use network_rust::bcast::BcastTransmitter;
use network_rust::localip::get_localip;
use std::io;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use serde::*;
use std::string;
use std::time::Duration;
use std::time::SystemTime;
use std::vec;
use std::collections::VecDeque;
use std::thread;
use std::thread::sleep;

pub struct Elev_Controller {
    add_queue: Sender<Order>,
    recive_queue: Receiver<Order>,
    queue: VecDeque<Order>,
    driver: ElevIo,
    stopped: bool,
    door_state: door_floor_state,
    last_floor: Floor,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ElevatorButtonEvent {
    pub action: ElevatorActions,
    pub floor: u8,
    pub origin: std::net::IpAddr
}

struct door_floor_state {
    timestamp_open: SystemTime,
    complete: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ElevatorActions {
    Cabcall,
    LobbyUpcall,
    LobbyDowncall
}

#[derive(Debug)]
pub struct Order {
    pub floor: u8
}

pub const BCAST_PORT: u16 = 26665;

fn init_elevator(elev_io: &ElevIo) {
    loop {
        match elev_io.get_floor_signal().unwrap() {
            Floor::At(_) => {
                elev_io.set_motor_dir(MotorDir::Stop).unwrap();
                break;
            }
            Floor::Between => {
                elev_io.set_motor_dir(MotorDir::Down).unwrap();
            }
        }
    }
}


impl Elev_Controller {
    pub fn new() -> io::Result<Self> {
        let (queue_sender, queue_reciver) = channel::<Order>();
        let que_obj: VecDeque<Order> = VecDeque::new();
        let elev_driver = ElevIo::new().expect("Connecting to elevator failed");
        init_elevator(&elev_driver);
        elev_driver.set_all_light(Light::Off).unwrap();
        let sys_time = SystemTime::now();
        let init_door_state = door_floor_state{timestamp_open: sys_time, complete: true} ;
        let current_floor = elev_driver.get_floor_signal().unwrap();
        let controller = Elev_Controller{add_queue: queue_sender, recive_queue: queue_reciver, queue: que_obj, driver:elev_driver, stopped: false, door_state:  init_door_state, last_floor: current_floor};
        Ok(controller)
    }
    
    pub fn handle_order(&mut self) {
        if !self.stopped {
            println!("[elev_controller]: {:?}", self.queue);
            match self.driver.get_floor_signal()
                        .expect("Get FloorSignal failed") {
                Floor::At(c_floor) => {
                    if !self.door_state.complete {
                        match self.door_state.timestamp_open.elapsed() {
                            Ok(time) => {
                                if time > Duration::from_secs(3) {                            
                                    self.driver.set_door_light(Light::Off).unwrap();
                                    self.door_state.complete = true;
                                    println!("[elev_controller] Door closed");
                                }
                            }
                            Err(e) => {
                                println!("[elev_controller]: Systime error occured {:?}", e);
                            }
                        }
                    } else {
                        self.driver.set_floor_light(Floor::At(c_floor)).unwrap();
                        match self.queue.front() {
                            Some(order) => {
                                //println!("[elev_controller] C: {:?} O: {:?}", c_floor, order.floor);   
                                if c_floor > order.floor{
                                    self.driver.set_motor_dir(MotorDir::Down).expect("Set MotorDir failed");
                                }
                                if c_floor < order.floor{
                                    self.driver.set_motor_dir(MotorDir::Up).expect("Set MotorDir failed");
                                }
                                if c_floor == order.floor{
                                    self.driver.set_motor_dir(MotorDir::Stop).expect("Set MotorDir failed");
                                    self.queue.pop_front();
                                    self.open_door();
                                    match self.last_floor {
                                        Floor::At(p_floor) => {
                                            //println!("[elev_controller] C: {:?} P: {:?}", c_floor, p_floor);
                                            if p_floor != c_floor {
                                                self.last_floor = self.driver.get_floor_signal().unwrap();
                                            }
                                        }
                                        Floor::Between => {
                                            self.last_floor = self.driver.get_floor_signal().unwrap();
                                        }
                                    }
                                }
                            }
                            None => {
                                self.driver.set_motor_dir(MotorDir::Stop).unwrap();
                            }
                        }
                    }
                    //println!("[elev_controller] C: {:?}", c_floor);   
                }
                // TODO: Make elevator handle floor logic if it starts in between state
                Floor::Between => {
                    match self.queue.front() {
                        Some(_) => {}
                        None => {
                            self.driver.set_motor_dir(MotorDir::Down).unwrap();
                        }
                    }
                }
            }
            match self.driver.get_stop_signal().unwrap(){
                Signal::High => {
                    self.driver.set_motor_dir(MotorDir::Stop).unwrap();
                    self.stopped = true;
                    self.driver.set_stop_light(Light::On).unwrap();
                }
                Signal::Low => {}
            }
        }
    }

    pub fn check_buttons(&mut self) {
        let broadcast = BcastTransmitter::new(BCAST_PORT).unwrap();
        for floor in 0..N_FLOORS {
            match self.driver.get_button_signal(Button::Internal(Floor::At(floor))).unwrap() {
                Signal::High => {
                    let data_block = ElevatorButtonEvent{action: ElevatorActions::Cabcall, floor: floor, origin:get_localip().unwrap() };
                    broadcast.transmit(&data_block).unwrap();
                }
                Signal::Low => {

                }
            }
            if floor != (N_FLOORS-1) {
                match self.driver.get_button_signal(Button::CallUp(Floor::At(floor))).expect("Unable to retrive hall up") {
                    Signal::High => {
                        let data_block = ElevatorButtonEvent{action: ElevatorActions::LobbyUpcall, floor: floor, origin:get_localip().unwrap() };
                        broadcast.transmit(&data_block).unwrap();
                    }
                    Signal::Low => {
    
                    }
                }
            }
            if floor != 0 {
                match self.driver.get_button_signal(Button::CallDown(Floor::At(floor))).expect("Unable to retrive hall down") {
                    Signal::High => {
                        let data_block = ElevatorButtonEvent{action: ElevatorActions::LobbyDowncall, floor: floor, origin:get_localip().unwrap() };
                        broadcast.transmit(&data_block).unwrap();
                    }
                    Signal::Low => {
    
                    }
                }
            }
            
        }
    }

    fn open_door(&mut self) {
        self.driver.set_door_light(Light::On).unwrap();
        println!("[elev_controller] Door open");
        self.door_state.complete = false;
        self.door_state.timestamp_open = SystemTime::now();

    }

    fn recive_order(&mut self) {
        let order = self.recive_queue.try_recv().unwrap();
        self.queue.push_back(order);
    }

    pub fn add_order(&mut self, order: Order) {
        self.queue.push_back(order);
    }
}