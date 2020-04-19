use elevator_driver::elev_driver::*;
use network_rust::bcast::BcastTransmitter;
use std::io;
use serde::*;
use std::thread;
use std::thread::sleep;
use std::sync::mpsc::Sender;
use std::time::Duration;
use std::time::SystemTime;
use std::collections::VecDeque;

pub struct ElevController {
    queue: VecDeque<Order>,
    driver: ElevIo,
    door_state: DoorState,
    previous_floor: Floor,
    internal_msg_sender: Sender<ButtonEvent>,
    elevator_id: u32,
    udp_broadcast_port: u16,
}

struct DoorState {
    timestamp_open: SystemTime,
    complete: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ButtonEvent {
    pub request: RequestType,
    pub order: Order,
    pub origin: u32
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RequestType {
    Request,
    Taken,
    Complete
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Order {
    pub floor: u8,
    pub order_type: ButtonType,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ButtonType {
    CabCall,
    HallUpCall,
    HallDownCall
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


impl ElevController {
    pub fn new(internal_message_sender: Sender<ButtonEvent>, elevator_id: u32, udp_broadcast_port: u16 , elevator_ip: &str, elevator_port: u16) -> io::Result<Self> {
        let que_obj: VecDeque<Order> = VecDeque::new();
        let elev_driver = ElevIo::new(elevator_ip, elevator_port).expect("Connecting to elevator failed");
        init_elevator(&elev_driver);
        elev_driver.set_all_light(Light::Off).unwrap();
        let sys_time = SystemTime::now();
        let init_door_state = DoorState{timestamp_open: sys_time, complete: true} ;
        let current_floor = elev_driver.get_floor_signal().unwrap();
        let controller = ElevController{queue: que_obj, driver:elev_driver, door_state:  init_door_state, previous_floor: current_floor, internal_msg_sender: internal_message_sender, elevator_id: elevator_id, udp_broadcast_port: udp_broadcast_port};
        Ok(controller)
    }
    
    pub fn handle_order(&mut self) {
        match self.driver.get_floor_signal()
                    .expect("Get FloorSignal failed") {
            Floor::At(c_floor) => {
                if !self.door_state.complete {
                    match self.door_state.timestamp_open.elapsed() {
                        Ok(time) => {
                            if time > Duration::from_secs(3) {                            
                                self.driver.set_door_light(Light::Off).unwrap();
                                self.door_state.complete = true;
                                //println!("[elev_controller] Door closed");
                            }
                        }
                        Err(e) => {
                            println!("[elev_controller]: Systime error occured {:?}", e);
                        }
                    }
                } else {
                    self.driver.set_floor_light(Floor::At(c_floor)).unwrap();
                    let mut clear_orders_at_floor: std::vec::Vec<Order> = vec![]; //used to clear all orders at the floor the elevator arrives at
                    let queue_clone=self.queue.clone();
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
                                for other_order in queue_clone{
                                    if other_order.floor == c_floor{
                                        clear_orders_at_floor.push(other_order.clone());
                                    }
                                }
                                self.complete_order_signal(order);
                                self.open_door();
                            } else {
                                // Completes cabcall orders which are on your way to the current order.
                                for other_order in queue_clone.clone(){
                                    match other_order.order_type{
                                        ButtonType::CabCall => {
                                            if other_order.floor == c_floor{
                                                self.driver.set_motor_dir(MotorDir::Stop).expect("Set MotorDir failed");
                                                clear_orders_at_floor.push(other_order.clone());
                                                self.open_door();
                                                for other_order in queue_clone.clone(){
                                                    match other_order.order_type{
                                                        ButtonType::CabCall =>{}
                                                        _ => {
                                                            if other_order.floor == c_floor{
                                                                clear_orders_at_floor.push(other_order.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            match self.previous_floor {
                                Floor::At(p_floor) => {
                                    if p_floor != c_floor {
                                        self.previous_floor = self.driver.get_floor_signal().unwrap();
                                    }
                                }
                                Floor::Between => {
                                    self.previous_floor = self.driver.get_floor_signal().unwrap();
                                }
                            }
                        }
                        None => {
                            self.driver.set_motor_dir(MotorDir::Stop).unwrap();
                        }
                    }
                    for order in clear_orders_at_floor {
                        let index = self.queue.iter().position(|x| *x == order).unwrap();
                        self.queue.remove(index);
                        self.complete_order_signal(&order);
                    }
                }
            }
            Floor::Between => {
                match self.queue.front() {
                    Some(order) => {
                        if self.get_previous_floor() > order.floor as isize{
                            self.driver.set_motor_dir(MotorDir::Down).expect("Set MotorDir failed");
                        }
                        if self.get_previous_floor() < order.floor as isize{
                            self.driver.set_motor_dir(MotorDir::Up).expect("Set MotorDir failed");
                        }
                    }
                    None => {
                        self.driver.set_motor_dir(MotorDir::Down).unwrap();
                    }
                }
            }
        }
    }

    pub fn get_current_floor(&self) -> isize { 
        match self.driver.get_floor_signal().unwrap() {
            Floor::At(num) => {
                num as isize
            }
            Floor::Between => {
                -1
            }
        }
    }

    pub fn get_previous_floor(&self) -> isize {
        match self.previous_floor {
            Floor::At(num) => {
                num as isize
            }
            Floor::Between => {
                -1
            }
        }
    }

    pub fn broadcast_active_buttons(&mut self) {
        for floor in 0..N_FLOORS {
            match self.driver.get_button_signal(Button::Internal(Floor::At(floor))).unwrap() {
                Signal::High => {
                    let order = Order{floor: floor, order_type: ButtonType::CabCall};
                    self.broadcast_order(order, RequestType::Request, self.elevator_id);
                }
                Signal::Low => {

                }
            }
            if floor != (N_FLOORS-1) {
                match self.driver.get_button_signal(Button::CallUp(Floor::At(floor))).expect("Unable to retrive hall up") {
                    Signal::High => {
                        let order = Order{floor: floor, order_type: ButtonType::HallUpCall};
                        self.broadcast_order(order, RequestType::Request, self.elevator_id);
                    }
                    Signal::Low => {
    
                    }
                }
            }
            if floor != 0 {
                match self.driver.get_button_signal(Button::CallDown(Floor::At(floor))).expect("Unable to retrive hall down") {
                    Signal::High => {
                        let order = Order{floor: floor, order_type: ButtonType::HallDownCall};
                        self.broadcast_order(order, RequestType::Request, self.elevator_id);
                    }
                    Signal::Low => {
    
                    }
                }
            } 
        }
    }

    fn open_door(&mut self) {
        self.driver.set_door_light(Light::On).unwrap();
        self.door_state.complete = false;
        self.door_state.timestamp_open = SystemTime::now();

    }

    fn complete_order_signal(&self, order: &Order) {
        let order_copy = order.clone();
        self.broadcast_order(order_copy, RequestType::Complete, self.elevator_id);
    }

    pub fn add_order(&mut self, order: Order) {
        let order_copy = order.clone();
        self.queue.push_back(order);
        self.broadcast_order(order_copy, RequestType::Taken, self.elevator_id);
    }

    pub fn delete_order(&mut self, order: &Order) {
        match self.queue.iter().position(|x| *x == *order){
            Some(index) => {
                self.queue.remove(index);
            },
            None => {
                println!("[elev_controller] Nothing to delete")
            }
        }
    }

    pub fn broadcast_order(&self, order: Order, request: RequestType, origin: u32) {
        let broadcast = BcastTransmitter::new(self.udp_broadcast_port).unwrap();
        let data_block_internal = ButtonEvent{request: request, order: order, origin:origin };
        //println!("{:?}: Broadcasting {:?}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap() , data_block_internal);
        let data_block_network = data_block_internal.clone();
        self.internal_msg_sender.send(data_block_internal).unwrap();
        thread::spawn(move || {
            for _ in 0..3 {
                broadcast.transmit(&data_block_network).unwrap();
                sleep(Duration::from_millis(50));
            }
        });
        
    }

    pub fn get_order_list(&self) -> VecDeque<Order> {
        let order_queue = self.queue.clone();
        order_queue
    }

    pub fn set_button_light_for_order(&mut self, action: &ButtonType, floor: Floor, light: Light) {
        match action {
            ButtonType::CabCall =>{
                self.driver.set_button_light(Button::Internal(floor), light).unwrap();
            }
            ButtonType::HallUpCall =>{
                self.driver.set_button_light(Button::CallUp(floor), light).unwrap();
            }
            ButtonType::HallDownCall =>{
                self.driver.set_button_light(Button::CallDown(floor), light).unwrap();
            }
        }
    }
}