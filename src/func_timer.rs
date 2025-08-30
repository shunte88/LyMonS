use std::time::Instant;

pub struct FunctionTimer {
    name: &'static str,
    start: Instant,
}

impl FunctionTimer {
     pub fn new(name: &'static str) -> Self {
        FunctionTimer {
            name,
            start: Instant::now(),
        }
    }
}

// This `Drop` implementation is called automatically when the `FunctionTimer` struct goes out of scope.
impl Drop for FunctionTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        println!("Function '{}' took: {:?}", self.name, duration);
    }
}
/*
fn do_more_work() {
    // The timer is created here...
    let _timer = FunctionTimer::new("do_more_work");
    
    // ...and is automatically dropped at the end of the function.
    std::thread::sleep(std::time::Duration::from_millis(100));
}

*/