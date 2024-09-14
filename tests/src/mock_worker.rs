use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    println!("Hello from program started from worker!");

    let args: Vec<String> = env::args().collect();
    let sleep_time = args.get(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(5);
    sleep(Duration::from_secs(sleep_time));
}
