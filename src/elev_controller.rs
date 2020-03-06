use elevator_driver::elev_driver::*;
use std::io;
use std::sync::mpsc::channel;
use std::sync::mpsc::{Receiver, Sender};
use std::string;
use std::time::Duration;
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
    last_floor: Floor,
}

pub struct Order {
    pub floor: u8
}



impl Elev_Controller {
    pub fn new() -> io::Result<Self> {
        let (queue_sender, queue_reciver) = channel::<Order>();
        let que_obj: VecDeque<Order> = VecDeque::new();
        let elev_driver = ElevIo::new().expect("Connecting to elevator failed");
        let current_floor = elev_driver.get_floor_signal().unwrap();
        elev_driver.set_all_light(Light::Off).unwrap();
        let controller = Elev_Controller{add_queue: queue_sender, recive_queue: queue_reciver, queue: que_obj, driver:elev_driver, stopped: false, last_floor: current_floor};
        Ok(controller)
    }
    
    pub fn handle_order(&mut self) {
        if !self.stopped {
            let mut is_order;
            match self.queue.front() {
                Some(value) => {
                    is_order = true;
                }
                None => {
                    is_order = false;
                }
            }
            match self.driver.get_floor_signal()
                        .expect("Get FloorSignal failed") {
                Floor::At(c_floor) => {
                    if !is_order {
                        self.driver.set_motor_dir(MotorDir::Stop).unwrap();
                    } else {
                        match self.queue.front() {
                            Some(order) => {
                                if c_floor > order.floor{
                                    self.driver.set_motor_dir(MotorDir::Down).expect("Set MotorDir failed");
                                }
                                if c_floor < order.floor{
                                    self.driver.set_motor_dir(MotorDir::Up).expect("Set MotorDir failed");
                                }
                                if c_floor == order.floor{
                                    self.driver.set_motor_dir(MotorDir::Stop).expect("Set MotorDir failed");
                                    match self.last_floor {
                                        Floor::At(p_floor) => {
                                            println!("[elev_controller] C: {:?} P: {:?}", c_floor, p_floor);
                                            if p_floor != c_floor {
                                                self.last_floor = self.driver.get_floor_signal().unwrap();
                                                self.open_door();
                                            }
                                        }
                                        Floor::Between => {
                                            self.last_floor = self.driver.get_floor_signal().unwrap();
                                            self.open_door();
                                        }
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                    println!("[elev_controller] C: {:?}", c_floor);   
                }
                // TODO: Make elevator handle floor logic if it starts in between state
                Floor::Between => {
                    if !is_order {
                        self.driver.set_motor_dir(MotorDir::Down);
                    }
                }
            }
            match self.driver.get_stop_signal().unwrap(){
                Signal::High => {
                    self.driver.set_motor_dir(MotorDir::Stop);
                    self.stopped = true;
                    self.driver.set_stop_light(Light::On);
                }
                Signal::Low => {}
            }
        }
    }

    fn open_door(&mut self) {
        self.driver.set_door_light(Light::On);
        sleep(Duration::from_secs(3)); // Should not be a delay
        self.driver.set_door_light(Light::Off);
    }

    fn recive_order(&mut self) {
        let order = self.recive_queue.try_recv().unwrap();
        self.queue.push_back(order);
    }
    pub fn add_order(&mut self, order: Order) {
        self.queue.push_back(order);
    }
}