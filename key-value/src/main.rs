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

use crate::store::KeyValue;

mod store;
mod test;

/*
* WireFormat {
    length,
    Op,
    value,
    key,
}
*/
#[repr(u8)]
#[derive(Debug)]
enum Op {
    GET = 1,
    SET = 2,
    DELETE = 3,
}

#[derive(Debug)]
enum Request<'a> {
    GET { key: &'a str },
    SET { key: &'a str, value: u32 },
    DELETE { key: &'a str },
}

#[repr(u8)]
#[derive(Debug)]
enum Status {
    Success = 0,
    Failure = 1,
}

#[derive(Debug)]
enum Response {
    GetResponse { status: Status, value: u32 },
    SetResponse { status: Status },
    DeleteResponse { status: Status, value: bool },
}

impl TryFrom<u8> for Op {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Op::GET),
            2 => Ok(Op::SET),
            3 => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {value}")),
        }
    }
}

impl FromStr for Op {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Op::GET),
            "SET" => Ok(Op::SET),
            "DELETE" => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {s}")),
        }
    }
}

impl TryFrom<&str> for Op {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            get => Ok(Op::GET),
            set => Ok(Op::SET),
            delete => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {value}")),
        }
    }
}

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
        let listen_buff = vec![0; 1024 as usize];
        let listen_buff_ptr = listen_buff.as_ptr();

        let num =
            unsafe { libc::recv(accepted_fd_socket, listen_buff_ptr as *mut c_void, 1024, 0) };
        if num == 0 {
            println!("received no data from the client");
            continue;
        } else {
            println!("received some data: {:?}", &listen_buff[0..50]);
        }
        let request = parse_request(&listen_buff);
        println!("{request:?}");
        let response = server.handle_request(request);
        let response_buff = parse_response(response);
        let _ = unsafe {
            libc::send(
                accepted_fd_socket,
                response_buff.as_ptr() as *const c_void,
                response_buff.len(),
                0,
            )
        };
        let _ = unsafe { libc::close(accepted_fd_socket) };
    }
    let _ = unsafe { libc::unlink(c_path.as_ptr()) };
}

// if it's a get/post/delete

// this takes in the byte stream and checks the op to figure out what to do with the request
// received some data: [0, 0, 0, 5, 49, 114, 105, 115, 104, 97, 98, 104, 52, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
fn parse_request(buffer: &Vec<u8>) -> Request {
    // this function should only person read operations on the original buffer
    // format: lengthopcodevaluekey
    let (len, rest) = buffer
        .split_first_chunk::<4>()
        .expect("buffer split at [length] failed");
    let length = u32::from_be_bytes(*len);
    let (op_byte, payload) = rest
        .split_first_chunk::<1>()
        .expect("buffer split at op failed");
    let op = Op::try_from(op_byte[0]).expect("unable to parse op byte code");
    let request: Request = match op {
        Op::GET => parse_get(payload, length),
        Op::SET => parse_set(payload, length),
        Op::DELETE => parse_delete(payload, length),
    };
    request
}

fn parse_response(response: Response) -> Vec<u8> {
    let mut result = Vec::new();
    match response {
        Response::GetResponse { status, value } => {
            result.push(Op::GET as u8);
            result.push(status as u8);
            result.extend_from_slice(&value.to_be_bytes());
        }
        Response::DeleteResponse { status, value } => {
            result.push(Op::DELETE as u8);
            result.push(status as u8);
            result.push(value as u8);
        }
        Response::SetResponse { status } => {
            result.push(Op::SET as u8);
            result.push(status as u8);
        }
    };
    result
}

fn parse_get(buffer: &[u8], length: u32) -> Request {
    let ptx = 0;
    let key = buffer[ptx..ptx + (length as usize)].try_into().unwrap();
    Request::GET {
        key: (str::from_utf8(key).unwrap()),
    }
}

fn parse_set(buffer: &[u8], length: u32) -> Request {
    let mut ptx = 0;
    let value = u32::from_be_bytes(buffer[ptx..ptx + 4].try_into().unwrap());
    ptx += 4;
    let key = str::from_utf8(buffer[ptx..ptx + (length as usize)].try_into().unwrap());
    Request::SET {
        key: (key.unwrap()),
        value: (value),
    }
    // return key, value
}

fn parse_delete(buffer: &[u8], lenght: u32) -> Request {
    let mut ptx = 0;
    let key = buffer[ptx..ptx + (lenght as usize)].try_into().unwrap();
    Request::DELETE {
        key: (str::from_utf8(key).unwrap()),
    }
}

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
    let mut result = Vec::new();
    let (op, key, value) = match request {
        Request::GET { key } => (Op::GET, key, Option::None),
        Request::SET { key, value } => (Op::SET, key, Option::Some(value)),
        Request::DELETE { key } => (Op::DELETE, key, Option::None),
        _ => panic!(""),
    };
    // encode the length first
    result.extend_from_slice(&(key.len() as u32).to_be_bytes());
    // op code
    result.push(op as u8);
    // value if available
    if value.is_some() {
        result.extend_from_slice(&value.unwrap().to_be_bytes());
    }
    // key now
    result.extend_from_slice(&key.as_bytes());
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
            listen(&mut server);
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
