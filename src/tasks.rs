use std::io;
use elevator_driver::*;
use std::sync::mpsc::*;
use std::vec::Vec;
use std::time::Duration;
use std::time::SystemTime;
use std::collections::VecDeque;
use rand::prelude::*;
use rand::Rng;

use crate::elev_controller;

#[derive(PartialEq, Clone, Debug)]
struct Task {
    order: elev_controller::Order,
    state: TaskStatemachineStates,
    taken: bool,
    complete: bool,
    complete_time: SystemTime,
    task_delay: CostFunctionDelay,
    origin_id: u32,
}

#[derive(PartialEq, Clone, Debug)]
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

pub struct TaskManager<'a> {
    elevator: elev_controller::ElevController<'a>,
    task_list: Vec<Task>,
    elevator_id: u32
}

impl Task {
    pub fn new(order: elev_controller::Order, origin_id: u32) -> io::Result<Self> {
        let default_delay = CostFunctionDelay {current_time: SystemTime::now(), waiting_time: Duration::from_secs(1)};
        let task = Task {order: order, state: TaskStatemachineStates::New, taken: false, complete: false, complete_time: SystemTime::now(), task_delay: default_delay, origin_id: origin_id};
        Ok(task)
    }
}

impl<'a> TaskManager<'a> {
    pub fn new(internal_sender: Sender<elev_controller::ElevatorButtonEvent>, elevator_id: u32, udp_broadcast_port: u16, elevator_ip: &str, elevator_port: u16) -> io::Result<Self> {
        let elev_controller = elev_controller::ElevController::new(internal_sender, elevator_id, udp_broadcast_port, elevator_ip, elevator_port).unwrap();
        let task_vec = Vec::new();
        let tsk_mgn = TaskManager {elevator: elev_controller, task_list: task_vec, elevator_id: elevator_id};
        Ok(tsk_mgn)
    }

    pub fn add_new_task(&mut self, order: elev_controller::Order, origin_id: u32) {
        let new_task = Task::new(order, origin_id).unwrap();
        let mut exist = false;
        for task in &mut self.task_list {
            if task.order == new_task.order {
                exist = true;
                if new_task.order.order_type == elev_controller::ElevatorActions::Cabcall && task.origin_id != new_task.origin_id {
                    exist = false;
                }
            }
        }
        if !exist {
            self.task_list.push(new_task);
        }
    }

    pub fn set_task_taken(&mut self, order: elev_controller::Order, origin_id: u32) {
        for task in &mut self.task_list {
            if task.order == order && order.order_type != elev_controller::ElevatorActions::Cabcall {
                task.taken = true;
            } else if task.order == order && order.order_type == elev_controller::ElevatorActions::Cabcall && task.origin_id == origin_id {
                task.taken = true;
            }
        }
    }

    pub fn set_task_complete(&mut self, order: elev_controller::Order, origin_id: u32) {
        for task in &mut self.task_list {
            if task.order == order && order.order_type != elev_controller::ElevatorActions::Cabcall {
                task.complete_time = SystemTime::now();
                task.complete = true;
            } else if task.order == order && order.order_type == elev_controller::ElevatorActions::Cabcall && task.origin_id == origin_id {
                task.complete_time = SystemTime::now();
                task.complete = true;
            }
        }
    }

    pub fn run_task_state_machine(&mut self) {
        self.elevator.handle_order();
        self.elevator.check_buttons();
        let mut task_delete_cleanup: std::vec::Vec<Task> = vec![];
        let tasks_copy = self.task_list.to_vec(); // This will make a copy of task_list before it iterates through it, the disadvantage here is that there is an delay in reactions in the cost function
        for task in &mut self.task_list {
            //println!("[tasks] {:?}", task);
            match task.state {
                TaskStatemachineStates::New => {
                    if task.origin_id != self.elevator_id && task.order.order_type == elev_controller::ElevatorActions::Cabcall {
                        task.state = TaskStatemachineStates::CheckKeepCabState;
                        task.task_delay.current_time = SystemTime::now();
                    } else {
                        task.state = TaskStatemachineStates::CostTake;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_take(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_last_floor(), self.elevator_id);
                        self.elevator.set_button_light_for_order(&task.order.order_type, elev_driver::Floor::At(task.order.floor), elev_driver::Light::On);
                        //println!("[task] Take delay {:?} {:?}", task.order, task.task_delay.waiting_time);
                    }
                }
                TaskStatemachineStates::CostTake => {
                    if task.taken {
                        //println!("[task]: Taken {:?}", task.order);
                        task.state = TaskStatemachineStates::CostComplete;
                        task.task_delay.current_time = SystemTime::now();
                        task.task_delay.waiting_time = TaskManager::cost_function_delay_complete(&task, &tasks_copy, &self.elevator.get_order_list(), self.elevator.get_current_floor(), self.elevator.get_last_floor(), self.elevator_id); 
                    } else if task.task_delay.current_time.elapsed().unwrap() > task.task_delay.waiting_time {
                        task.state = TaskStatemachineStates::Take;
                        //println!("[task]: Taking {:?}", task.order);
                    }
    
                }
                TaskStatemachineStates::CheckKeepCabState => {
                    if task.complete {
                        task.state = TaskStatemachineStates::Complete;
                    } else {
                        // Spam order on UDP every 10th secound
                        if task.task_delay.current_time.elapsed().unwrap() > Duration::from_secs(10) {
                            println!("[task]: Spamming UDP {:?} {:?}", task.order, task.origin_id);
                            task.task_delay.current_time = SystemTime::now();
                            let order_clone = task.order.clone();
                            self.elevator.broadcast_order(order_clone, elev_controller::RequestType::Request, task.origin_id);
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
                        //println!("[task]: Taking uncomplete task {:?} {:?}", task.order, task.origin_id);
                        task.state = TaskStatemachineStates::Take;
                    }
                }
                TaskStatemachineStates::CheckComplete => {
                    if task.complete {
                        task.state = TaskStatemachineStates::Complete;
                        if task.order.order_type != elev_controller::ElevatorActions::Cabcall {
                            self.elevator.delete_order(&task.order);
                        } 
                    }
    
                }
                TaskStatemachineStates::Complete => {
                    ////println!("[tasks] Completed {:?}", task);
                    if (task.order.order_type == elev_controller::ElevatorActions::Cabcall && task.origin_id == self.elevator_id) || task.order.order_type != elev_controller::ElevatorActions::Cabcall {
                        //println!("[task]: turning off {:?} {:?}", task.order, task.origin_id);
                        self.elevator.set_button_light_for_order(&task.order.order_type, elev_driver::Floor::At(task.order.floor), elev_driver::Light::Off);
                    }
                    task_delete_cleanup.push(task.clone());
                }
            }
        }
        for task in task_delete_cleanup {
            if task.complete_time.elapsed().unwrap() > Duration::from_secs(5) || task.order.order_type != elev_controller::ElevatorActions::Cabcall {
                let index = self.task_list.iter().position(|x| *x == task).unwrap();
                self.task_list.remove(index);
            }  
        }
    }

    fn cost_function_delay_take(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize, elev_id: u32) -> Duration {
        // Cost function Wodo magic

        // Number of floors, Distance between elevator and call, Direction of elevator

        ////println!("Current Task: {:?}\n task_queue: {:?}\n elev_queue {:?}", task_order, task_queue, elev_queue);
        let mut rng = thread_rng();
        let mut score = 1; // Higher is better, must be > 0
        //println!("[COST_DEBUG]: {:?}", task_order);
        let mut number_of_others_tasks = 0;
        let mut number_of_elevator_orders = 0;
        for task in task_queue {
            if task.state == TaskStatemachineStates::CostTake || task.state == TaskStatemachineStates::New {
                number_of_others_tasks += 1;
            }
        }
        for elev_orders in elev_queue {
            number_of_elevator_orders += 1;
        }
        match elev_queue.front() {
            Some(elev_current_doing) => {
                //There are other orders in the elevator
                let direction = TaskManager::direction_of_call(current_floor, last_floor);
                //override variables used to manipulate cost function delay
                let mut score_override=1;
                let mut ip_score_override=1;
                let mut long_queue_delay_override=1;
                let incoming_order = &task_order;
                match elev_current_doing.order_type {
                    elev_controller::ElevatorActions::Cabcall => {
                        if direction == Direction::Down && last_floor > task_order.order.floor as isize ||
                        direction == Direction::Up && last_floor < task_order.order.floor as isize{
                            //Elevator moving towards order
                            score=(elev_driver::N_FLOORS as isize + 2) - (task_order.order.floor as isize - last_floor).abs();
                        }
                        else{
                            //Elevator moving away from order
                            score=1;
                        }
                        ip_score_override=0; 
                        long_queue_delay_override=0;
                    }
                    elev_controller::ElevatorActions::LobbyDowncall => {
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
                    elev_controller::ElevatorActions::LobbyUpcall => {
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
                if incoming_order.order.order_type==elev_controller::ElevatorActions::Cabcall{
                    ip_score_override=0;
                    long_queue_delay_override=0;
                    score_override=0;
                } 
                if incoming_order.order.floor==elev_current_doing.floor{
                    ip_score_override=0;
                    long_queue_delay_override=0;
                    score_override=0;
                }
                
                let ip_score=elev_id;
                let delay =2000+(5000/score)*score_override+2500 * number_of_elevator_orders*long_queue_delay_override+150*ip_score as isize*ip_score_override;
                // basis_delay+score_delay    +        amount_of_order_delay      +                         unique_ip_delay
                println!("[COST_DEBUG]: score_some_queue {:?} other_tasks {:?} elev_orders {:?}", score, number_of_others_tasks, number_of_elevator_orders);
                println!("[COST_DEBUG]: delay {:?}", delay);

                Duration::from_millis(delay as u64)
            }
            None => {

                   //There is no other orders in the elevator
                    let mut delay =1000;
                    let ip_score= elev_id;
                    let mut distance_score=0;
                
                
                    //let random_number :u8 = rand::thread_rng().gen_range(1,10);
                    //delay += (random_number as u64) * 10;
                
                    let current_order =&task_order.order;
    
                    if current_order.order_type==elev_controller::ElevatorActions::Cabcall{
                        delay=20;
                    }
    
                    else{
                        distance_score =(current_floor-current_order.floor as isize).abs();
                        delay=delay+500*distance_score as u64 +150*ip_score as u64;
    
                    }
                    println!("DELAY :   {:?}",delay);
                    println!("TASK Q: {:?}",task_queue[0].order.order_type);
                    println!("Elev Q: {:?}",elev_queue.front());
                    //delay += number_of_others_tasks*500;
                    //println!("[COST_DEBUG]: no_queue {:?}", delay);
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

    fn cost_function_delay_complete(task_order: &Task, task_queue: &Vec<Task>, elev_queue: &VecDeque<elev_controller::Order>, current_floor: isize, last_floor: isize, elev_id: u32) -> Duration {
        // Cost function Wodo magic
        Duration::from_secs(elev_driver::N_FLOORS as u64 * 3) + TaskManager::cost_function_delay_take(task_order, task_queue, elev_queue, current_floor, last_floor, elev_id)
    }
}

