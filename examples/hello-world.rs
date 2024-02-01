extern crate tiny_http;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use sysinfo::{Components, Disks, Networks, System};


#[cfg(feature = "memory_monitoring")]
fn main() {
    use rustls::server;

    thread::spawn(monitor_memory_usage);
    let server = Arc::new(tiny_http::Server::http_memory("0.0.0.0:9976", Some(4),  Some(2)).unwrap());
    println!("Now listening on port 9976");
    for rq in server.incoming_requests() {
        let response = tiny_http::Response::from_string("hello world".to_string());
        let _ = rq.respond(response);
    }
}

#[cfg(not(feature = "memory_monitoring"))]
fn main() {
    thread::spawn(monitor_memory_usage);

    let server = Arc::new(tiny_http::Server::http("0.0.0.0:9976", Some(4)).unwrap());
    println!("Now listening on port 9975");
    for rq in server.incoming_requests() {
        let response = tiny_http::Response::from_string("hello world".to_string());
        let _ = rq.respond(response);
    }

    // let mut handles = Vec::new();
    //
    // for _ in 0..4 {
    //     let server = server.clone();
    //
    //     handles.push(thread::spawn(move || {
    //         for rq in server.incoming_requests() {
    //             let response = tiny_http::Response::from_string("hello world".to_string());
    //             let _ = rq.respond(response);
    //         }
    //     }));
    // }
    //
    // for h in handles {
    //     h.join().unwrap();
    // }
}
fn monitor_memory_usage() {
    loop {
        std::thread::sleep(Duration::from_secs(10)); // 每30秒触发一次
        let mut sys = System::new_all();
        // First we update all information of our `System` struct.
        sys.refresh_all();
        let bytes_to_mb = |bytes| bytes as f64 / (1024 * 1024) as f64;

        println!("=> system:");
        // RAM and swap information:
        println!("total memory: {:.2} MB", bytes_to_mb(sys.total_memory()));
        println!("total used memory: {:.2} MB", bytes_to_mb(sys.used_memory()));
        for (pid, process) in sys.processes() {
            if pid.as_u32() == std::process::id() {
                println!(
                    "[{pid}] {} memory: {:.2} MB",
                    process.name(),
                    bytes_to_mb(process.memory())
                );
            }
        }
    }
}
