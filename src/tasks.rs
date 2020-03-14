use std::io;
use elevator_driver::*;
use network_rust::bcast::BcastReceiver;
use std::sync::mpsc::*;
use std::vec::Vec;
use std::time::Duration;
use std::time::SystemTime;

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

#[derive(Clone, Debug)]
enum TaskStatemachineStates {
    new,
    cost_take,
    take,
    cost_complete,
    check_complete,
    complete,
}

pub struct TaskManager {
    elevator: elev_controller::ElevController,
    task_list: Vec<Task>,
}

impl Task {
    pub fn new(order: elev_controller::Order, ip_origin: std::net::IpAddr) -> io::Result<Self> {
        let default_delay = CostFunctionDelay {current_time: SystemTime::now(), waiting_time: Duration::from_secs(1)};
        let task = Task {order: order, state: TaskStatemachineStates::new, taken: false, complete: false, task_delay: default_delay, ip_origin: ip_origin};
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
                exist = true;
                if task.complete {
                    task.complete = false;
                    task.taken = false;
                    task.state = TaskStatemachineStates::new;
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
                TaskStatemachineStates::new => {
                    task.state = TaskStatemachineStates::cost_take;
                    task.task_delay.current_time = SystemTime::now();
                    task.task_delay.waiting_time = TaskManager::cost_function_delay_take(&task.order, &tasks_copy); 
                }
                TaskStatemachineStates::cost_take => {
                    if task.taken {
                        task.state = TaskStatemachineStates::cost_complete;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_complete(&task.order, &tasks_copy); 
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = TaskStatemachineStates::take;
                    }
    
                }
                TaskStatemachineStates::take => {
                    let order_clone = task.order.clone();
                    self.elevator.add_order(order_clone);
                    task.state = TaskStatemachineStates::check_complete;
                }
                TaskStatemachineStates::cost_complete => {
                    if task.complete {
                        task.state = TaskStatemachineStates::complete;
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = TaskStatemachineStates::take;
                    }
                }
                TaskStatemachineStates::check_complete => {
                    if task.complete {
                        task.state = TaskStatemachineStates::complete;
                    }
    
                }
                TaskStatemachineStates::complete => {
                    //println!("[tasks] Completed {:?}", task);
                }
            }
        }
    }

    fn cost_function_delay_take(task_order: &elev_controller::Order, task_queue: &Vec<Task>) -> Duration {
        // Cost function Wodo magic
        Duration::from_millis(10)
    }
    fn cost_function_delay_complete(task_order: &elev_controller::Order, task_queue: &Vec<Task>) -> Duration {
        // Cost function Wodo magic
        Duration::from_secs(2)
    }
}

