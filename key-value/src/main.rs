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
use crate::net::Protocol;
use crate::net::client_connect;
use crate::net::listen;
use crate::store::KeyValue;
use std::sync::Arc;
use std::sync::Mutex;

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

fn main() {
    let args: Vec<String> = env::args().collect();
    let parse_args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut server = Arc::new(Mutex::new(store::KeyValue::new(String::from("data/"))));
    match parse_args.as_slice() {
        [_, "start", "--", "ipc", protocol] => {
            let protocol = Protocol::try_from(*protocol);
            listen(server, protocol.expect("unknown protocol"));
        }
        [_, "client", "--", "ipc", protocol, rest @ ..] => match rest {
            ["get", key] => client_connect(
                Request::GET {
                    key: (key.to_string()),
                },
                Protocol::try_from(*protocol).expect("unknown protocol"),
            ),
            ["set", key, value] => client_connect(
                Request::SET {
                    key: key.to_string(),
                    value: u32::from_str(value).unwrap(),
                },
                Protocol::try_from(*protocol).expect("unknown protocol"),
            ),
            ["delete", key] => client_connect(
                Request::DELETE {
                    key: key.to_string(),
                },
                Protocol::try_from(*protocol).expect("unknown protocol"),
            ),
            _ => println!("client subcommand parsing failed"),
        },
        [_, "get", key] => {
            let ret = server.lock().unwrap().get(key);
            println!("the value for {key} is {}", ret.unwrap_or(0));
        }
        [_, "set", key, value] => {
            server
                .lock()
                .unwrap()
                .set(key.to_string(), u32::from_str(value).unwrap());
        }
        [_, "stop"] => {
            server.lock().unwrap().stop();
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
