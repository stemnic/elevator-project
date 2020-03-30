use std::io;
use elevator_driver::*;
use network_rust::bcast::BcastReceiver;
use network_rust::localip::get_localip;
use std::sync::mpsc::*;
use std::vec::Vec;
use std::time::Duration;
use std::time::SystemTime;
use std::collections::VecDeque;
use rand::prelude::*;

use crate::elev_controller;

#[derive(Clone, Debug)]
struct Task {
    order: elev_controller::Order,
    state: TaskStatemachineStates,
    taken: bool,
    complete: bool,
    task_delay: CostFunctionDelay,
    ip_origin: std::net::IpAddr,
}

#[derive(Clone, Debug)]
struct CostFunctionDelay {
    current_time: SystemTime,
    waiting_time: Duration,
}

#[derive(PartialEq, Clone, Debug)]
enum TaskStatemachineStates {
    New,
    CostTake,
    Take,
    CheckKeepCabState,
    CostComplete,
    CheckComplete,
    Complete,
}

#[derive(PartialEq, Debug)]
enum Direction {
    Up,
    Down
}

pub struct TaskManager {
    elevator: elev_controller::ElevController,
    task_list: Vec<Task>,
}

impl Task {
    pub fn new(order: elev_controller::Order, ip_origin: std::net::IpAddr) -> io::Result<Self> {
        let default_delay = CostFunctionDelay {current_time: SystemTime::now(), waiting_time: Duration::from_secs(1)};
        let task = Task {order: order, state: TaskStatemachineStates::New, taken: false, complete: false, task_delay: default_delay, ip_origin: ip_origin};
        Ok(task)
    }
}

impl TaskManager {
    pub fn new() -> io::Result<Self> {
        let elev_controller = elev_controller::ElevController::new().unwrap();
        let task_vec = Vec::new();
        let tsk_mgn = TaskManager {elevator: elev_controller, task_list: task_vec};
        Ok(tsk_mgn)
    }

    pub fn add_new_task(&mut self, order: elev_controller::Order, ip_origin: std::net::IpAddr) {
        let new_task = Task::new(order, ip_origin).unwrap();
        let mut exist = false;
        for task in &mut self.task_list {
            if task.order == new_task.order {
                if task.order.order_type == elev_controller::ElevatorActions::Cabcall {
                    if task.ip_origin == ip_origin {
                        exist = true;
                        if task.complete {
                            task.complete = false;
                            task.taken = false;
                            task.state = TaskStatemachineStates::New;
                            task.ip_origin = ip_origin;
                        }
                    } else {
                        exist = false;
                    }
                } else {
                    exist = true;
                    if task.complete {
                        task.complete = false;
                        task.taken = false;
                        task.state = TaskStatemachineStates::New;
                        task.ip_origin = ip_origin;
                    }
                }
            }
        }
        if !exist {
            self.task_list.push(new_task);
        }
    }

    pub fn set_task_taken(&mut self, order: elev_controller::Order) {
        for task in &mut self.task_list {
            if task.order == order {
                task.taken = true;
            }
        }
    }

    pub fn set_task_complete(&mut self, order: elev_controller::Order) {
        for task in &mut self.task_list {
            if task.order == order {
                task.complete = true;
            }
        }
    }

    pub fn run_task_state_machine(&mut self) {
        self.elevator.handle_order();
        self.elevator.check_buttons();
        let tasks_copy = self.task_list.to_vec(); // This will make a copy of task_list before it iterates through it, the disadvantage here is that there is an delay in reactions in the cost function
        for task in &mut self.task_list {
            //println!("[tasks] {:?}", task);
            match task.state {
                TaskStatemachineStates::New => {
                    if task.ip_origin != get_localip().unwrap() && task.order.order_type == elev_controller::ElevatorActions::Cabcall {
                        task.state = TaskStatemachineStates::CheckKeepCabState;
                        task.task_delay.current_time = SystemTime::now();
                    } else {
                        task.state = TaskStatemachineStates::CostTake;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_take(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_last_floor());
                        println!("[tasks] {:?}", task);
                    }
                }
                TaskStatemachineStates::CostTake => {
                    if task.taken {
                        task.state = TaskStatemachineStates::CostComplete;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_complete(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_last_floor()); 
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = TaskStatemachineStates::Take;
                    }
    
                }
                TaskStatemachineStates::CheckKeepCabState => {
                    if task.complete {
                        task.state = TaskStatemachineStates::Complete;
                    } else {
                        // Spam order on UDP every 3rd secound
                        if task.task_delay.current_time.elapsed().unwrap() > Duration::from_secs(3) {
                            task.task_delay.current_time = SystemTime::now();
                            let order_clone = task.order.clone();
                            self.elevator.broadcast_order(order_clone, elev_controller::RequestType::Request, task.ip_origin);
                        }
                    }
                }
                TaskStatemachineStates::Take => {
                    let order_clone = task.order.clone();
                    self.elevator.add_order(order_clone);
                    task.state = TaskStatemachineStates::CheckComplete;
                }
                TaskStatemachineStates::CostComplete => {
                    if task.complete {
                        task.state = TaskStatemachineStates::Complete;
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = TaskStatemachineStates::Take;
                    }
                }
                TaskStatemachineStates::CheckComplete => {
                    if task.complete {
                        task.state = TaskStatemachineStates::Complete;
                    }
    
                }
                TaskStatemachineStates::Complete => {
                    //println!("[tasks] Completed {:?}", task);
                }
            }
        }
    }

    fn cost_function_delay_take(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize) -> Duration {
        // Cost function Wodo magic

        // Number of floors, Distance between elevator and call, Direction of elevator

        //println!("Current Task: {:?}\n task_queue: {:?}\n elev_queue {:?}", task_order, task_queue, elev_queue);
        let mut rng = thread_rng();
        let mut score = 0; // Higher is better
        match elev_queue.front() {
            Some(elev_current_doing) => {
                //There are other orders in the elevator
                let direction = TaskManager::direction_of_call(current_floor, last_floor);
                match elev_current_doing.order_type {
                    elev_controller::ElevatorActions::Cabcall => {
                        score = 100;
                    }
                    elev_controller::ElevatorActions::LobbyDowncall => {
                        if direction == Direction::Down && last_floor > task_order.order.floor as isize {
                            // Elevator moving to order /w same direction
                            score = (elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        } else if direction == Direction::Up && last_floor > task_order.order.floor as isize {
                            // Elevator moving to order /w opposit direction
                            score = (elev_driver::N_FLOORS as isize + 1) - (task_order.order.floor as isize - last_floor).abs();
                        } else {
                            // Away from order
                            score = 1;
                        }
                    }
                    elev_controller::ElevatorActions::LobbyUpcall => {
                        if direction == Direction::Up && last_floor < task_order.order.floor as isize {
                            // Elevator moving to order /w same direction
                            score = (elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        } else if direction == Direction::Down && last_floor < task_order.order.floor as isize {
                            // Elevator moving to order /w opposit direction
                            score = (elev_driver::N_FLOORS as isize + 1) - (task_order.order.floor as isize - last_floor).abs();
                        } else {
                            // Away from order
                            score = 1;
                        }
                    }
                }

                let mut delay = Duration::from_millis(5000/(score as u64));

                delay
            }
            None => {
                //There is no other orders in the elevator
                let mut delay = 50;
                if task_order.ip_origin == get_localip().unwrap() {
                    delay = 1
                }
                let mut number_of_others = 0;
                for task in task_queue {
                    if task.state == TaskStatemachineStates::CostTake || task.state == TaskStatemachineStates::New {
                        number_of_others += 1;
                    }
                }
                delay += number_of_others;
                Duration::from_millis(delay)
            }
        }
    }

    fn direction_of_call(going_to: isize, last_floor: isize) -> Direction { 
        let mut dir = Direction::Up;
        if going_to - last_floor > 0 {
            dir = Direction::Up;
        } else {
            dir = Direction::Down;
        }
        dir
    }

    fn cost_function_delay_complete(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize) -> Duration {
        // Cost function Wodo magic
        Duration::from_secs(elev_driver::N_FLOORS as u64 * 3) + TaskManager::cost_function_delay_take(task_order, task_queue, elev_queue, current_floor, last_floor)
    }
}

