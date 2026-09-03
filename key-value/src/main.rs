/*
 * Build a key value server with incremental goals following the cs664
 */
#![allow(warnings)]
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

mod layout;
mod net;
mod store;
#[cfg(test)]
mod test;
// splits the key based on dot delimiter and returns back its components
// use rsplit_once instead of generic split as it cuts the string in two halves (prefix | suffix)
fn split_key(key: &String) -> Option<(&str, &str)> {
    let result = key.rsplit_once(".");
    result
}

fn client_connect(request: Request) {
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
    let result = frame_encoder(request);
    // need a buffer now
    let _ = unsafe { libc::send(socket_fd, result.as_ptr() as *const c_void, result.len(), 0) };
    let output_buf: Vec<u8> = vec![0; 1024];
    let _ = unsafe { libc::recv(socket_fd, output_buf.as_ptr() as *mut c_void, 1024, 0) };
    println!("server returned: {output_buf:?}")
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let parse_args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut server = store::KeyValue::new(String::from("data/"));
    match parse_args.as_slice() {
        [_, "start"] => {
            crate::net::listen(&mut server);
        }
        [_, "client", rest @ ..] => match rest {
            ["get", key] => client_connect(Request::GET { key: (key) }),
            ["set", key, value] => client_connect(Request::SET {
                key: key,
                value: u32::from_str(value).unwrap(),
            }),
            ["delete", key] => client_connect(Request::DELETE { key: key }),
            _ => println!("client subcommand parsing failed"),
        },
        [_, "get", key] => {
            let ret = server.get(key);
            println!("the value for {key} is {}", ret.unwrap_or(0));
        }
        [_, "set", key, value] => {
            server.set(key.to_string(), u32::from_str(value).unwrap());
        }
        [_, "stop"] => {
            server.stop();
        }
        _ => panic!("can't parse arguments"),
    };
    // match server.get("sample.rishabh2") {
    //     Some(key) => println!("value from rishabh2: {}", key),
    //     None => {}
    // }
    // server.set(String::from("rishabh4"), 89);
    // let all_keys = server.get_all();
    // dbg!(all_keys);
    // // server.delete_key("rishabh4");
    // dbg!(server);
}
