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
    stopped: bool,
    door_state: DoorFloorState,
    last_floor: Floor,
    internal_msg_sender: Sender<ElevatorButtonEvent>,
    elevator_id: u32,
    udp_broadcast_port: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ElevatorButtonEvent {
    pub request: RequestType,
    pub order: Order,
    pub origin: u32
}

struct DoorFloorState {
    timestamp_open: SystemTime,
    complete: bool
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ElevatorActions {
    Cabcall,
    LobbyUpcall,
    LobbyDowncall
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
    pub order_type: ElevatorActions,
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
    pub fn new(internal_message_sender: Sender<ElevatorButtonEvent>, elevator_id: u32, udp_broadcast_port: u16 , elevator_ip: &str, elevator_port: u16) -> io::Result<Self> {
        let que_obj: VecDeque<Order> = VecDeque::new();
        let elev_driver = ElevIo::new(elevator_ip, elevator_port).expect("Connecting to elevator failed");
        init_elevator(&elev_driver);
        elev_driver.set_all_light(Light::Off).unwrap();
        let sys_time = SystemTime::now();
        let init_door_state = DoorFloorState{timestamp_open: sys_time, complete: true} ;
        let current_floor = elev_driver.get_floor_signal().unwrap();
        let controller = ElevController{queue: que_obj, driver:elev_driver, stopped: false, door_state:  init_door_state, last_floor: current_floor, internal_msg_sender: internal_message_sender, elevator_id: elevator_id, udp_broadcast_port: udp_broadcast_port};
        Ok(controller)
    }
    
    pub fn handle_order(&mut self) {
        //println!("{:?}", self.queue);
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
                                for task in queue_clone{
                                    if task.floor == c_floor{
                                        clear_orders_at_floor.push(task.clone());
                                    }
                                }
                                self.complete_order_signal(order);
                                self.open_door();
                            } else {
                                for task in queue_clone.clone(){
                                    match task.order_type{
                                        ElevatorActions::Cabcall => {
                                            if task.floor == c_floor{
                                                self.driver.set_motor_dir(MotorDir::Stop).expect("Set MotorDir failed");
                                                let index = self.queue.iter().position(|x| *x == task).unwrap();
                                                for task in queue_clone.clone(){
                                                    match task.order_type{
                                                        ElevatorActions::Cabcall =>{}
                                                        _ => {
                                                            if task.floor == c_floor{
                                                                clear_orders_at_floor.push(task.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                                self.queue.remove(index);
                                                self.complete_order_signal(&task);
                                                self.open_door();
                                                ////println!("[Elev_controller]: Stopped here!")
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            match self.last_floor {
                                Floor::At(p_floor) => {
                                    ////println!("[elev_controller] C: {:?} P: {:?}", c_floor, p_floor);
                                    if p_floor != c_floor {
                                        self.last_floor = self.driver.get_floor_signal().unwrap();
                                    }
                                }
                                Floor::Between => {
                                    self.last_floor = self.driver.get_floor_signal().unwrap();
                                }
                            }
                        }
                        None => {
                            self.driver.set_motor_dir(MotorDir::Stop).unwrap();
                        }
                    }
                    for task in clear_orders_at_floor {
                        let index = self.queue.iter().position(|x| *x == task).unwrap();
                        self.queue.remove(index);
                        self.complete_order_signal(&task);
                    }
                }
                ////println!("[elev_controller] C: {:?}", c_floor);   
            }
            // TODO: Make elevator handle floor logic if it starts in between state
            Floor::Between => {
                match self.queue.front() {
                    Some(order) => {
                        if self.get_last_floor() > order.floor as isize{
                            self.driver.set_motor_dir(MotorDir::Down).expect("Set MotorDir failed");
                        }
                        if self.get_last_floor() < order.floor as isize{
                            self.driver.set_motor_dir(MotorDir::Up).expect("Set MotorDir failed");
                        }
                    }
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

    pub fn get_last_floor(&self) -> isize {
        match self.last_floor {
            Floor::At(num) => {
                num as isize
            }
            Floor::Between => {
                -1
            }
        }
    }

    pub fn check_buttons(&mut self) {
        for floor in 0..N_FLOORS {
            match self.driver.get_button_signal(Button::Internal(Floor::At(floor))).unwrap() {
                Signal::High => {
                    let order = Order{floor: floor, order_type: ElevatorActions::Cabcall};
                    self.broadcast_order(order, RequestType::Request, self.elevator_id);
                }
                Signal::Low => {

                }
            }
            if floor != (N_FLOORS-1) {
                match self.driver.get_button_signal(Button::CallUp(Floor::At(floor))).expect("Unable to retrive hall up") {
                    Signal::High => {
                        let order = Order{floor: floor, order_type: ElevatorActions::LobbyUpcall};
                        self.broadcast_order(order, RequestType::Request, self.elevator_id);
                    }
                    Signal::Low => {
    
                    }
                }
            }
            if floor != 0 {
                match self.driver.get_button_signal(Button::CallDown(Floor::At(floor))).expect("Unable to retrive hall down") {
                    Signal::High => {
                        let order = Order{floor: floor, order_type: ElevatorActions::LobbyDowncall};
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
        //println!("[elev_controller]: Door open");
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
        let data_block_internal = ElevatorButtonEvent{request: request, order: order, origin:origin };
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

    pub fn set_button_light_for_order(&mut self, action: &ElevatorActions, floor: Floor, light: Light) {
        match action {
            ElevatorActions::Cabcall =>{
                self.driver.set_button_light(Button::Internal(floor), light).unwrap();
            }
            ElevatorActions::LobbyUpcall =>{
                self.driver.set_button_light(Button::CallUp(floor), light).unwrap();
            }
            ElevatorActions::LobbyDowncall =>{
                self.driver.set_button_light(Button::CallDown(floor), light).unwrap();
            }
        }
    }
}