/*
 * Build a key value server with incremental goals following the cs664
 */

use libc;
use std::ffi::CString;
use std::ffi::c_void;

struct KeyValue<'a> {
    capacity: u32,
    fd: usize,
    buffer: &'a mut [u8],
}

impl KeyValue<'_> {
    pub fn new(capacity: u32, filename: String) -> Self {
        let c_filename = CString::new(filename).expect("Failed to create CString");
        Self {
            capacity,
            fd: unsafe {libc::open(c_filename.as_ptr(), libc::O_CREAT | libc::O_APPEND | libc::O_EXCL, libc::O_RDWR) as usize},
            buffer: &mut [],
        }
    }
    // keeping the key data type to be strictly a string for now
    pub fn get(&self, key: String) -> u8 {
        let chunk = 1024;
        let ptx = 0;
        let mut buffer_start = self.buffer.as_ptr();
        while true {
            let read_bytes = unsafe {libc::read(self.fd as i32, buffer_start as *mut c_void, chunk)};
            if read_bytes == 0 {
                panic!("Not reading anything")
            }
            unsafe {buffer_start += (ptx + chunk)};
            chunk += 1024;
        }
        self.buffer[]
    }
}

fn main() {
    println!("Hello, world!");
}
