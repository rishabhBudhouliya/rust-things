/*
 * Build a key value server with incremental goals following the cs664
 */

use libc;
use std::collections::HashMap;
use std::ffi::CString;
use std::ffi::c_void;

struct KeyValue<'a> {
    capacity: u32,
    fd: u32,
    buffer: &'a mut [u8],
    map: HashMap<String, u32>,
}

impl KeyValue<'_> {
    pub fn new(capacity: u32, filename: String) -> Self {
        let c_filename = CString::new(filename).expect("Failed to create CString");
        let fd = unsafe {
            libc::open(
                c_filename.as_ptr(),
                libc::O_CREAT | libc::O_APPEND | libc::O_EXCL,
                libc::O_RDWR,
            ) as u32
        };
        let map = parse_buffer(fd);
        Self {
            capacity,
            fd,
            map,
            buffer: &mut [],
        }
    }
    // keeping the key data type to be strictly a string for now
    // pub fn get(&self, key: String) -> u8 {
    //     let chunk = 1024;
    //     let mut buffer_start = self.buffer.as_ptr();
    //     while true {
    //         let read_bytes =
    //             unsafe { libc::read(self.fd as i32, buffer_start as *mut c_void, chunk) };
    //         if read_bytes == 0 {
    //             break;
    //         }
    //         buffer_start = self.buffer[chunk..];
    //         chunk += 1024;
    //     }
    // }

    // "key:value/nkey:value/nkey:value"
}

fn parse_buffer(file_d: u32) -> HashMap<String, u32> {
    let mut chunk = 4096;
    let buffer = &mut [];
    let mut buffer_start: *const u8 = buffer.as_ptr();
    let mut map: HashMap<String, u32> = HashMap::new();
    while true {
        let read_bytes = unsafe { libc::read(file_d as i32, buffer_start as *mut c_void, chunk) };
        if read_bytes == 0 {
            break;
        }
        let mut iter = buffer.split(|num| *num == b'\n');
        // this should give us a list of records
        for record in iter {
            let mut key_value_iter = record.split(|num| *num == b':');
            // add a check on whether the iter has length = 2 or not
            let key = key_value_iter.next().unwrap();
            let value = key_value_iter.next().unwrap();
            map.insert(
                u32::from_be_bytes(key.try_into().unwrap()).to_string(),
                u32::from_be_bytes(value.try_into().unwrap()),
            );
        }
        buffer_start = buffer[chunk..].as_ptr();
        chunk += 1024;
    }
    return map;
}

fn main() {
    println!("Hello, world!");
}
