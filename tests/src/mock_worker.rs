use std::thread::sleep;
use std::time::Duration;

fn main() {
    println!("Hello from program started from worker!");
    sleep(Duration::from_secs(5));
}
