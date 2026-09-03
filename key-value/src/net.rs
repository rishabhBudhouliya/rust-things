use libc;
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

pub fn listen(server: &mut KeyValue) {
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
    let _ = unsafe { libc::unlink(c_path.as_ptr()) };
}

// serve the connection
fn accept(accepted_socket: i32, server: &mut KeyValue) {
    let listen_buff = vec![0; 1024 as usize];
    let listen_buff_ptr = listen_buff.as_ptr();
    loop {
        let ret = unsafe { libc::recv(accepted_socket, listen_buff_ptr as *mut c_void, 1024, 0) };
        if ret == 0 {
            break;
        }
        let request = crate::layout::parse_request(&listen_buff);
        let response = server.handle_request(request);
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
