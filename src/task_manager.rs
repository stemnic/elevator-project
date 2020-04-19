use std::io;
use elevator_driver::*;
use std::sync::mpsc::*;
use std::vec::Vec;
use std::time::Duration;
use std::time::SystemTime;
use std::collections::VecDeque;

use crate::elev_controller;

#[derive(PartialEq, Clone, Debug)]
struct Task {
    order: elev_controller::Order,
    state: States,
    taken: bool,
    complete: bool,
    complete_time: SystemTime,
    task_delay: CostFunctionDelay,
    origin_id: u32,
}

#[derive(PartialEq, Clone, Debug)]
enum States {
    New,
    DelayTake,
    Take,
    CabWatchdog,
    CompleteWatchdog,
    CheckLocalComplete,
    Complete,
}

#[derive(PartialEq, Clone, Debug)]
struct CostFunctionDelay {
    current_time: SystemTime,
    waiting_time: Duration,
}

#[derive(PartialEq, Debug)]
enum Direction {
    Up,
    Down
}

pub struct TaskManager {
    elevator: elev_controller::ElevController,
    task_list: Vec<Task>,
    elevator_id: u32
}

impl Task {
    pub fn new(order: elev_controller::Order, origin_id: u32) -> io::Result<Self> {
        let default_delay = CostFunctionDelay {current_time: SystemTime::now(), waiting_time: Duration::from_secs(1)};
        let task = Task {order: order, state: States::New, taken: false, complete: false, complete_time: SystemTime::now(), task_delay: default_delay, origin_id: origin_id};
        Ok(task)
    }
}

impl TaskManager {
    pub fn new(internal_sender: Sender<elev_controller::ButtonEvent>, elevator_id: u32, udp_broadcast_port: u16, elevator_ip: &str, elevator_port: u16) -> io::Result<Self> {
        let elev_controller = elev_controller::ElevController::new(internal_sender, elevator_id, udp_broadcast_port, elevator_ip, elevator_port).unwrap();
        let task_vec = Vec::new();
        let manager = TaskManager {elevator: elev_controller, task_list: task_vec, elevator_id: elevator_id};
        Ok(manager)
    }

    pub fn add_new_task(&mut self, order: elev_controller::Order, origin_id: u32) {
        let new_task = Task::new(order, origin_id).unwrap();
        let mut task_exist = false;
        for task in &mut self.task_list {
            if task.order == new_task.order {
                task_exist = true;
                if new_task.order.order_type == elev_controller::ButtonType::CabCall && task.origin_id != new_task.origin_id {
                    task_exist = false;
                }
            }
        }
        if !task_exist {
            self.task_list.push(new_task);
        }
    }

    pub fn set_task_taken(&mut self, order: elev_controller::Order, origin_id: u32) {
        for task in &mut self.task_list {
            if task.order == order && order.order_type != elev_controller::ButtonType::CabCall {
                task.taken = true;
            } else if task.order == order && order.order_type == elev_controller::ButtonType::CabCall && task.origin_id == origin_id {
                task.taken = true;
            }
        }
    }

    pub fn set_task_complete(&mut self, order: elev_controller::Order, origin_id: u32) {
        for task in &mut self.task_list {
            if task.order == order && order.order_type != elev_controller::ButtonType::CabCall {
                task.complete_time = SystemTime::now();
                task.complete = true;
            } else if task.order == order && order.order_type == elev_controller::ButtonType::CabCall && task.origin_id == origin_id {
                task.complete_time = SystemTime::now();
                task.complete = true;
            }
        }
    }

    pub fn run_state_machine(&mut self) {
        self.elevator.handle_order();
        self.elevator.broadcast_active_buttons();
        let mut task_delete_cleanup: std::vec::Vec<Task> = vec![];
        let tasks_copy = self.task_list.to_vec();
        for task in &mut self.task_list {
            match task.state {
                States::New => {
                    if task.origin_id != self.elevator_id && task.order.order_type == elev_controller::ButtonType::CabCall {
                        task.state = States::CabWatchdog;
                        task.task_delay.current_time = SystemTime::now();
                    } else {
                        task.state = States::DelayTake;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_take(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_previous_floor(), self.elevator_id);
                        self.elevator.set_button_light_for_order(&task.order.order_type, elev_driver::Floor::At(task.order.floor), elev_driver::Light::On);
                    }
                }
                States::DelayTake => {
                    if task.taken {
                        task.state = States::CompleteWatchdog;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_complete(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_previous_floor(), self.elevator_id); 
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = States::Take;
                    }
                }
                // Monitors if any hallcalls orders have timed out after a elevator has taken it
                States::CompleteWatchdog => {
                    if task.complete {
                        task.state = States::Complete;
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = States::Take;
                    }
                }
                // Monitors other elevators cabcalls and broadcasts them until they are complete
                States::CabWatchdog => {
                    if task.complete {
                        task.state = States::Complete;
                    } else {
                        if task.task_delay.current_time.elapsed().unwrap() > Duration::from_secs(10) {
                            println!("[task_manager]: Repeating CabCall Order {:?} {:?}", task.order, task.origin_id);
                            task.task_delay.current_time = SystemTime::now();
                            let order_clone = task.order.clone();
                            self.elevator.broadcast_order(order_clone, elev_controller::RequestType::Request, task.origin_id);
                        }
                    }
                }
                States::Take => {
                    let order_clone = task.order.clone();
                    self.elevator.add_order(order_clone);
                    task.state = States::CheckLocalComplete;
                }
                States::CheckLocalComplete => {
                    if task.complete {
                        task.state = States::Complete;
                        if task.order.order_type != elev_controller::ButtonType::CabCall {
                            self.elevator.delete_order(&task.order);
                        } 
                    }
                }
                States::Complete => {
                    if (task.order.order_type == elev_controller::ButtonType::CabCall && task.origin_id == self.elevator_id) || 
                            task.order.order_type != elev_controller::ButtonType::CabCall {
                        self.elevator.set_button_light_for_order(&task.order.order_type, elev_driver::Floor::At(task.order.floor), elev_driver::Light::Off);
                    }
                    task_delete_cleanup.push(task.clone());
                }
            }
        }
        for task in task_delete_cleanup {
            if task.complete_time.elapsed().unwrap() > Duration::from_secs(5) || task.order.order_type != elev_controller::ButtonType::CabCall {
                let index = self.task_list.iter().position(|x| *x == task).unwrap();
                self.task_list.remove(index);
            }  
        }
    }

    fn cost_function_delay_take(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize, elev_id: u32) -> Duration {
        // Number of floors, Distance between elevator and call, Direction of elevator

        
        let ip_score=elev_id;
        let direction = TaskManager::direction_of_call(current_floor, last_floor);
        let incoming_order = &task_order;

        

        let mut number_of_elevator_orders = 0;
        for _elev_orders in elev_queue {
            number_of_elevator_orders += 1;
        }
        // Override variables used to manipulate cost function delay
        let mut score_override=1;
        let mut ip_score_override=1;
        let mut long_queue_delay_override=1;

        match elev_queue.front() {
            Some(elev_current_doing) => {
                // There are other orders in the elevator

                let score; // Higher is better, must be > 0
                match elev_current_doing.order_type {
                    elev_controller::ButtonType::CabCall => {
                        if direction == Direction::Down && last_floor > task_order.order.floor as isize ||
                        direction == Direction::Up && last_floor < task_order.order.floor as isize{
                            // Elevator moving towards order
                            score = (elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        }
                        else{
                            // Elevator moving away from order
                            score = 1;
                        }

                    }
                    elev_controller::ButtonType::HallDownCall => {
                        if direction == Direction::Down && last_floor > task_order.order.floor as isize {
                            // Elevator moving to order /w same direction
                            score = (elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        } else if direction == Direction::Up && last_floor < task_order.order.floor as isize {
                            // Elevator moving to order /w opposit direction
                            score = (elev_driver::N_FLOORS as isize + 1) - (task_order.order.floor as isize - last_floor).abs();
                        } else {
                            // Away from order
                            score = 1;
                        }
                    }
                    elev_controller::ButtonType::HallUpCall => {
                        if direction == Direction::Up && last_floor < task_order.order.floor as isize {
                            // Elevator moving to order /w same direction
                            score = (elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        } else if direction == Direction::Down && last_floor > task_order.order.floor as isize {
                            // Elevator moving to order /w opposit direction
                            score = (elev_driver::N_FLOORS as isize + 1) - (task_order.order.floor as isize - last_floor).abs();
                        } else {
                            // Away from order
                            score = 1;
                        }
                    }
                }
                if incoming_order.order.order_type==elev_controller::ButtonType::CabCall || incoming_order.order.floor==elev_current_doing.floor{
                    ip_score_override=0;
                    long_queue_delay_override=0;
                    score_override=0;
                } 
                let delay =2000+(5000/score)* score_override+2500 * number_of_elevator_orders * long_queue_delay_override+150 * ip_score as isize * ip_score_override;
                // basis_delay+score_delay    +        amount_of_order_delay      +                         unique_ip_delay
                println!("[COST_DEBUG]: score_some_queue {:?} elev_orders {:?}", score, number_of_elevator_orders);
                println!("[COST_DEBUG]: delay {:?}", delay);

                Duration::from_millis(delay as u64)
            }
            None => {

                   //There is no other orders in the elevator
                    let mut delay =1000;

                    let current_order =&task_order.order;
                    if current_order.order_type==elev_controller::ButtonType::CabCall{
                        delay=20;
                    }
                    else{
                        let distance_score =(current_floor-current_order.floor as isize).abs();
                        delay=delay+500*distance_score as u64 +150*ip_score as u64;
    
                    }
                    println!("[COST_DEBUG]: DELAY : {:?}",delay);
                    println!("[COST_DEBUG]: TASK Q: {:?}",task_queue[0].order.order_type);
                    println!("[COST_DEBUG]: Elev Q: {:?}",elev_queue.front());
                    Duration::from_millis(delay)
            }
        }
    }

    fn direction_of_call(going_to: isize, last_floor: isize) -> Direction { 
        let dir;
        if going_to - last_floor > 0 {
            dir = Direction::Up;
        } else {
            dir = Direction::Down;
        }
        dir
    }

    fn cost_function_delay_complete(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize, elev_id: u32) -> Duration {
        Duration::from_secs(elev_driver::N_FLOORS as u64 * 3) + TaskManager::cost_function_delay_take(task_order, task_queue, elev_queue, current_floor, last_floor, elev_id)
    }
}