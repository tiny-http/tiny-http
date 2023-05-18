extern crate tiny_http;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

use libc::{__rlimit_resource_t, getrlimit, rlimit, setrlimit, RLIMIT_NOFILE, RLIMIT_NPROC};

fn set_limit(limit: __rlimit_resource_t, value: u64) {
    let mut current = rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    assert_eq!(0, unsafe { getrlimit(limit, &mut current) });
    current.rlim_cur = value;
    assert_eq!(0, unsafe { setrlimit(limit, &mut current) });
}

struct ServerProcess {
    pid: libc::pid_t,
}

impl ServerProcess {
    fn start(setup: impl FnOnce()) -> (std::net::SocketAddr, ServerProcess) {
        let listener = TcpListener::bind("0.0.0.0:0").unwrap();
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            setup();
            let server = tiny_http::Server::from_listener(listener, None).unwrap();
            for req in server.incoming_requests() {
                req.respond(tiny_http::Response::empty(204)).unwrap();
            }
            std::process::exit(0);
        } else {
            let addr = listener.local_addr().unwrap();
            drop(listener);
            (addr, Self { pid })
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.pid, libc::SIGKILL);
            libc::waitpid(self.pid, std::ptr::null_mut(), 0);
        }
    }
}

fn make_request_with_keep_alive(addr: std::net::SocketAddr) -> std::io::Result<TcpStream> {
    TcpStream::connect(addr).and_then(|mut s| {
        let mut buf = [0; 1024];
        write!(
            s,
            "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n"
        )?;
        s.read(&mut buf)?;
        Ok(s)
    })
}

#[test]
fn survives_fd_limit_1() {
    let (addr, _server) = ServerProcess::start(|| {
        // Each connection creates to file-descriptors. Let's test with a limit one off
        // from survives_fd_limit_2 to trigger out-of-fd at both places in the code
        set_limit(RLIMIT_NOFILE, 8);
    });

    assert_survives_hitting_concurrency_limits(addr);
}

#[test]
fn survives_fd_limit_2() {
    let (addr, _server) = ServerProcess::start(|| {
        // Each connection creates to file-descriptors. Let's test with a limit one off
        // from survives_fd_limit_1 to trigger out-of-fd at both places in the code
        set_limit(RLIMIT_NOFILE, 7);
    });

    assert_survives_hitting_concurrency_limits(addr);
}

fn assert_survives_hitting_concurrency_limits(addr: SocketAddr) {
    let clients: Vec<_> = (0..10)
        .map(|_| make_request_with_keep_alive(addr))
        .collect();
    assert!(clients.iter().any(Result::is_err));

    drop(clients);

    assert!(TcpStream::connect(addr).is_ok());
}

#[test]
fn survives_proc_limit() {
    let (addr, _server) = ServerProcess::start(|| {
        set_limit(RLIMIT_NPROC, 7);
    });

    assert_survives_hitting_concurrency_limits(addr);
}
