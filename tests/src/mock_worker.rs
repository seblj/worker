use std::env;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let unique_id = args.get(1).expect("No unique ID provided");
    let sleep_time = args.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(5);

    println!("Hello from program started from worker!");
    println!(
        "Mock worker {} starting, will sleep for {} seconds",
        unique_id, sleep_time
    );
    sleep(Duration::from_secs(sleep_time));
    println!("Mock worker {} finished", unique_id);
}
