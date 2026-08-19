/*
 * Build a key value server with incremental goals following the cs664
 */

use libc;
use std::collections::HashMap;
use std::ffi::CString;
use std::ffi::c_void;
use std::mem::MaybeUninit;

mod test;

#[derive(Debug)]
struct KeyValue {
    fd: u32,
    map: HashMap<String, u32>,
}

impl KeyValue {
    pub fn new(filename: String) -> Self {
        let c_filename = CString::new(filename).expect("Failed to create CString");
        let fd = unsafe {
            libc::open(
                c_filename.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_APPEND,
                0o666,
            ) as i32
        };
        if fd == -1 {
            panic!("couldn't open fileserver");
        }
        let map = parse_buffer(fd as u32);
        Self {
            fd: (fd as u32),
            map,
        }
    }

    // goal 2: define get/set functions for the KeyValue struct
    // we should not promise an owner value, instead, it should be a peek/reference into the storage
    pub fn get(&self, key: String) -> Option<&u32> {
        return self.map.get(&key);
    }

    // set requires two steps
    // 1) mutate the in-memory hashmap
    // 2) mutate the on-disk hashmap
    // implement deduplication
    pub fn set(&mut self, key: String, value: u32) {
        let x = &key;
        let y = &value.to_string();
        let stored = format!("{x}:{y}\n");
        if self.map.get(x) == Some(&value) {
            println!("Value already exists");
            return;
        }
        self.map.insert(key, value);
        let written_buffer = stored.into_bytes();
        let size_buffer = written_buffer.len();
        let ptr: *const u8 = written_buffer.as_ptr();
        let _ = unsafe { libc::write(self.fd as i32, ptr as *mut c_void, size_buffer as usize) };
    }
}

fn parse_buffer(file_d: u32) -> HashMap<String, u32> {
    let mut stat: MaybeUninit<libc::stat> = MaybeUninit::uninit();
    let ret = unsafe { libc::fstat(file_d as i32, stat.as_mut_ptr()) };
    let size = if ret != -1 {
        unsafe { stat.assume_init() }.st_size
    } else {
        0
    };
    let buffer = vec![0; size as usize];
    let buffer_start: *const u8 = buffer.as_ptr();
    let mut map: HashMap<String, u32> = HashMap::new();
    let read_bytes =
        unsafe { libc::read(file_d as i32, buffer_start as *mut c_void, size as usize) };
    dbg!(read_bytes);
    if read_bytes == 0 {
        panic!("Couldn't read anything");
    }
    let iter = buffer.split(|num| *num == b'\n');
    // this should give us a list of records
    // [100, 201, 123, 212, 214, :, 144, 124, 412, 421]
    for record in iter {
        if record.is_empty() {
            print!("Encountered an empty record");
            break;
        }
        let mut key_value_iter = record.split(|num| *num == b':');
        // add a check on whether the iter has length = 2 or not
        let key = String::from_utf8((key_value_iter.next().unwrap()).try_into().unwrap()).unwrap();
        let value = str::from_utf8(key_value_iter.next().unwrap()).unwrap();
        map.insert(key, u32::from_str_radix(value, 10).unwrap());
    }
    return map;
}

fn main() {
    let mut server = KeyValue::new("sample.txt".try_into().unwrap());
    let ret = server.get(String::from("rishabh2")).unwrap();
    println!("value from rishabh2: {}", ret);
    server.set(String::from("rishabh4"), 89);
    dbg!(server);
}
