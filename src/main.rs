use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::env;
use std::sync::mpsc::SyncSender;

struct Fork {}

struct Uptime { // struct sent to main thread to monitor philosophers death
    id: u8,
    eat_time: Instant,
}

struct Settings { // Table settings
    time_to_die: u8,
    time_to_eat: u8,
    time_to_sleep: u8,
}

struct Philosopher {
    id: u8,
    time_to_eat: u8,
    time_to_sleep: u8,
}

impl Philosopher {
    fn new(id: u8, time_to_eat: u8, time_to_sleep: u8)-> Philosopher {
        Philosopher { id, time_to_eat, time_to_sleep }
    }
    fn eat(&self) {
        println!("{:?} {} is eating", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), self.id);
        thread::sleep(Duration::from_secs(self.time_to_eat as u64));
    }
    fn think(&self) {
        println!("{:?} {} is thinking", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), self.id);
    }
    fn sleep(&self) {
        println!("{:?} {} is sleeping", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), self.id);
        thread::sleep(Duration::from_secs(self.time_to_sleep as u64));
    }
    fn routine(self, left_fork: Arc<Mutex<Fork>>, right_fork: Arc<Mutex<Fork>>, signal_rx: Arc<Mutex<mpsc::Receiver<u8>>>, uptime_tx: SyncSender<Uptime>)-> thread::JoinHandle<()>{
        thread::spawn(move|| loop {
            // receive an Err if a philosophers dies because it dropped the sender
            if let Err(mpsc::TryRecvError::Disconnected) = signal_rx.lock().unwrap().try_recv() {
                break;
            }
            // lock both forks enabling the philosopher to eat
            let (left_lock, right_lock) = (left_fork.lock().unwrap(), right_fork.lock().unwrap());
            println!("{:?} {} has taken its both forks", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), self.id);
            // send uptime to main thread through sync_sender
            uptime_tx.send(Uptime{ id: self.id, eat_time: Instant::now() }).unwrap();
            self.eat();
            // drop both forks locks
            drop(left_lock);
            drop(right_lock);

            self.sleep();

            self.think();
        })
    }
}

struct Table {
    size: u8,
    philosophers: Vec<Philosopher>, // philosopher entities
    forks: Vec<Arc<Mutex<Fork>>>, // forks entities
    handles: Vec<thread::JoinHandle<()>>, // philosophers routine handles
    settings: Settings, // table settings
}

impl Table {
    fn new(size: u8, settings: Settings)-> Table {
        let mut philosophers: Vec<Philosopher> = Vec::with_capacity(size as usize);
        let mut forks: Vec<Arc<Mutex<Fork>>> = Vec::with_capacity(size as usize);
        let handles: Vec<thread::JoinHandle<()>> = Vec::with_capacity(size as usize);

        for id in 0..size { // populate philosophers and forks vectors
            philosophers.push(Philosopher::new(id, settings.time_to_eat, settings.time_to_sleep));
            forks.push(Arc::new(Mutex::new(Fork{})));
        }

        Table { size, philosophers, forks, handles, settings }
    }

    fn start(mut self) {
        let (signal_tx, signal_rx) = mpsc::channel(); // init channel used to shut down threads if a philosopher died
        let mut signal_tx = Some(signal_tx);
        let signal_rx = Arc::new(Mutex::new(signal_rx));

        let (uptime_tx, uptime_rx) = mpsc::sync_channel(self.size as usize); // init sync_channel used to update the last philosopher meal time


        for p in self.philosophers { // init routines == thread
            let id = p.id as usize;
            self.handles.push(p.routine(Arc::clone(&self.forks[id]), Arc::clone(&self.forks[if id < self.size as usize-1 {id + 1} else {0}]), Arc::clone(&signal_rx), uptime_tx.clone()))
        }

        let mut tracking = vec![Instant::now(); self.size as usize]; // init vector tracking last meal time
        'monitoring: loop {
            if let Ok(update) = uptime_rx.try_recv() { // on receive of an Uptime{} element, update matching tracking vector index
                let to_update = tracking.get_mut(update.id as usize).unwrap();
                *to_update = update.eat_time;
            }
            for (id, ts) in tracking.iter().enumerate() { // look for a died philosopher
                if *ts < Instant::now() - Duration::from_secs(self.settings.time_to_die as u64){
                    println!("{:?} {} died", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), id);
                    break 'monitoring; // on death break out of loop to shutdown
                }
            }
        }
        drop(signal_tx.take()); // drop signal == thread triggers shut down on Err receive from try_recv()

        for handle in self.handles { // join all threads handles
            handle.join().unwrap();
        }
    }
}

fn main() {
    let mut args = env::args();
    args.next();
    let size: u8 = args.next().unwrap().parse().unwrap();
    let settings = Settings {
        time_to_die: args.next().unwrap().parse().unwrap(), // time span in seconds a philosopher can spend without eating once before dying
        time_to_eat: args.next().unwrap().parse().unwrap(), // time it takes a philosopher to die
        time_to_sleep: args.next().unwrap().parse().unwrap(), // time it takes a philosopher to sleep
        // round_amount: args.next().unwrap().parse().unwrap(),
    };

    let table = Table::new(size, settings);
    table.start(); // start experiment
}
