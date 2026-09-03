use libc;
use std::collections::VecDeque;
use std::env;
use std::ffi::CString;
use std::ffi::c_void;
use std::io;
use std::str::FromStr;
use std::time::Duration;
use std::time::Instant;

use crate::layout::Op;
use crate::layout::Request;
use crate::layout::Response;
use crate::layout::Status;
use crate::layout::frame_encoder;
use crate::store::KeyValue;

#[derive(PartialEq, Debug)]
pub enum Protocol {
    unix,
    tcp,
}

impl TryFrom<&str> for Protocol {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "unix" => Ok(Protocol::unix),
            "tcp" => Ok(Protocol::tcp),
            _ => Err(format!("unknown/unsupported protocol {value}")),
        }
    }
}

pub fn listen(server: &mut KeyValue, protocol: Protocol) {
    let socket_fd = if protocol == Protocol::tcp {
        let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        let mut sockaddr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
        let port = unsafe { libc::getuid() } + 2000;
        sockaddr.sin_family = libc::AF_INET as u8;
        sockaddr.sin_addr = libc::in_addr {
            s_addr: libc::INADDR_ANY,
        };
        sockaddr.sin_port = u16::to_be(port as u16);
        let raw_sock_ptr: *const libc::sockaddr =
            (&sockaddr as *const libc::sockaddr_in) as *const libc::sockaddr;
        let ret = unsafe {
            libc::bind(
                socket_fd,
                raw_sock_ptr,
                std::mem::size_of::<libc::sockaddr_in>() as u32,
            )
        };

        if ret == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            println!("unable to bind to socket");
            return;
        }
        socket_fd
    } else {
        let socket_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        let mut sockaddr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let path = String::from("server.sock");
        let c_path = CString::new(path).expect("Failed to create CString");
        let mut counter = 0;
        let s_bytes_arr = c_path.as_bytes();
        for ele in sockaddr.sun_path.iter_mut() {
            if counter >= s_bytes_arr.len() {
                break;
            }
            *ele = s_bytes_arr[counter] as i8;
            counter += 1;
        }
        sockaddr.sun_family = libc::AF_UNIX as u8;
        // now that sockaddrr is ready, let's invoke the bind syscall
        let raw_sock_ptr: *const libc::sockaddr =
            (&sockaddr as *const libc::sockaddr_un) as *const libc::sockaddr;

        let ret = unsafe {
            libc::bind(
                socket_fd,
                raw_sock_ptr,
                std::mem::size_of::<libc::sockaddr_un>() as u32,
            )
        };

        if ret == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            println!("unable to bind to socket");
            return;
        }
        socket_fd
    };

    // start listening, keep the queue for backlog 0
    let ret = unsafe { libc::listen(socket_fd, 0) };
    if ret == -1 {
        let err = io::Error::last_os_error();
        dbg!(err);
        println!("unable to start listening to socket");
        return;
    }
    let start = Instant::now();
    // accept() blocks the caller until a connection is present unless non blocking sockets are used
    loop {
        if Instant::now() - start >= Duration::from_mins(5) {
            println!("server going to shutdown");
            break;
        }
        let accepted_fd_socket =
            unsafe { libc::accept(socket_fd, std::ptr::null_mut(), std::ptr::null_mut()) };
        if accepted_fd_socket == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            println!("unable to accept connections");
        }
        accept(accepted_fd_socket, server);
        let _ = unsafe { libc::close(accepted_fd_socket) };
    }
    let path = String::from("server.sock");
    let c_path = CString::new(path).expect("Failed to create CString");
    let _ = unsafe { libc::unlink(c_path.as_ptr()) };
}

// serve the connection
fn accept(accepted_socket: i32, server: &mut KeyValue) {
    let listen_buff = vec![0; 1024 as usize];
    let listen_buff_ptr = listen_buff.as_ptr();
    let mut tail_buffer = Vec::new();
    let mut result_buffer = VecDeque::new();
    loop {
        let ret = unsafe { libc::recv(accepted_socket, listen_buff_ptr as *mut c_void, 1024, 0) };
        if ret == 0 || ret == -1 {
            break;
        }
        crate::layout::parse_request(
            &listen_buff[..ret as usize],
            &mut tail_buffer,
            &mut result_buffer,
        );
        while result_buffer.len() != 0 {
            let request = result_buffer.pop_front();
            if request.is_none() {
                continue;
            }
            let response = server.handle_request(request.unwrap());
            let response_buff = crate::layout::parse_response(response);
            let _ = unsafe {
                libc::send(
                    accepted_socket,
                    response_buff.as_ptr() as *const c_void,
                    response_buff.len(),
                    0,
                )
            };
        }
    }
}

pub fn client_connect(request: Request, protocol: Protocol) {
    let socket_fd = if protocol == Protocol::tcp {
        let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
        let mut sockaddr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
        let port = unsafe { libc::getuid() } + 2000;
        sockaddr.sin_family = libc::AF_INET as u8;
        sockaddr.sin_addr = libc::in_addr {
            // 127.0.0.1
            s_addr: u32::to_be(0x7f000001),
        };
        sockaddr.sin_port = u16::to_be(port as u16);
        let raw_sock_ptr: *const libc::sockaddr =
            (&sockaddr as *const libc::sockaddr_in) as *const libc::sockaddr;
        let ret = unsafe {
            libc::connect(
                socket_fd,
                raw_sock_ptr,
                std::mem::size_of::<libc::sockaddr_in>() as u32,
            )
        };
        if ret == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            panic!("couldn't connect");
        }
        socket_fd
    } else {
        let connect_path = "server.sock"; // will need to parameterize this
        let socket_fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        // need to construct a sockaddr
        let mut sockaddr: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let path = String::from(connect_path);
        let c_path = CString::new(path).expect("Failed to create CString");
        let mut counter = 0;
        let s_bytes_arr = c_path.as_bytes();
        for ele in sockaddr.sun_path.iter_mut() {
            if counter >= s_bytes_arr.len() {
                break;
            }
            *ele = s_bytes_arr[counter] as i8;
            counter += 1;
        }
        sockaddr.sun_family = libc::AF_UNIX as u8;
        // now that sockaddrr is ready, let's invoke the bind syscall
        let raw_sock_ptr: *const libc::sockaddr =
            (&sockaddr as *const libc::sockaddr_un) as *const libc::sockaddr;

        let ret = unsafe {
            libc::connect(
                socket_fd,
                raw_sock_ptr,
                std::mem::size_of::<libc::sockaddr_un>() as u32,
            )
        };
        if ret == -1 {
            panic!("couldn't connect");
        }
        socket_fd
    };
    let result = frame_encoder(request);
    // need a buffer now
    let _ = unsafe { libc::send(socket_fd, result.as_ptr() as *const c_void, result.len(), 0) };
    let output_buf: Vec<u8> = vec![0; 1024];
    let _ = unsafe { libc::recv(socket_fd, output_buf.as_ptr() as *mut c_void, 1024, 0) };
    println!("server returned: {output_buf:?}")
}
