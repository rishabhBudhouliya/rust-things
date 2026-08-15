/*
 * Design a ring buffer backed by a shared memory and protected by a semaphore
 * Learn how to create and maintain a shared memory in std io
 * Learn how to create, link and unlink a mmap reference
 * (why do I need an mmap invocation to link the shared memory segment?)
 * Learn how to use a kernel backed semaphore
 */

use std::ffi::c_void;
use std::io;

const HEADER: u8 = 8;

/*
*  shm_open — create/open the segment, returns fd
   ftruncate — size it (creator only)
   mmap — map it, MAP_SHARED
   close — fd, right after mapping
   idea: ref count usage of ring buffer and drop it/destroy the structure when no one's using it
*/
#[derive(Debug)]
pub struct RingBuffer<'a> {
    capacity: u32,
    shmid: usize,
    buffer: &'a mut [u8], // exclusive mutable reference is desired
    head: u32,
    tail: u32,
}

/*
 * Think about what can make a ring buffer lock free:
 * Approach 1: Can my front and back pointers be atomically updated?
 */
impl RingBuffer<'_> {
    pub fn new(size: u32) -> io::Result<Self> {
        let name = c"/ring6";
        let shm_fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600) };
        if shm_fd == -1 {
            let err = io::Error::last_os_error();
            return Err(err);
            // if err.raw_os_error() != Some(libc::EEXIST) {
            //     return Err(err);
            // }
            panic!("unable to allocate a file descriptor to shared memory")
        }
        let ret = unsafe { libc::ftruncate(shm_fd, (size + 8) as i64) };
        if ret == -1 {
            panic!("unable to size shm")
        }
        // how do handle a c_void pointer? How do I represent an opaque struct in Rust?
        //  https://doc.rust-lang.org/nomicon/ffi.html#representing-opaque-structs
        let map_buffer_start =
            // TODO: figure out the first argument
            unsafe { libc::mmap(0 as *mut c_void, size.try_into().unwrap(), libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, shm_fd, 0) as *mut u8};
        if map_buffer_start as isize == -1 {
            panic!("unable to mmap to the shared memory segment")
        }
        let mapped_buffer: &mut [u8] =
            unsafe { std::slice::from_raw_parts_mut(map_buffer_start, size.try_into().unwrap()) };
        // close the shm file descriptor
        let head = u32::from_be_bytes(mapped_buffer[0..4].try_into().unwrap());
        // let head = u32::from_be_bytes(mapped_buffer[0..4].try_into().unwrap());
        let tail = u32::from_be_bytes(mapped_buffer[4..8].try_into().unwrap());
        Ok(Self {
            capacity: size,
            shmid: shm_fd as usize,
            buffer: mapped_buffer,
            head,
            tail,
        })
    }

    pub fn close(&mut self) {
        unsafe { libc::close(self.shmid as i32) };
        let name = std::ffi::CString::from(c"/ring");
        let ret = unsafe { libc::shm_unlink(name.as_ptr()) };
        if ret != 0 {
            panic!("can't unlink the shared memory segment")
        }
    }

    pub fn push(&mut self, value: u8) -> usize {
        if self.is_full() {
            panic!("the buffer is full")
        }
        let index = (8 + (self.get_tail() % self.capacity));
        let destination: &mut u8 = (&mut self.buffer[index as usize]).try_into().unwrap();
        *destination = value;
        self.set_tail(&self.get_tail() + 1);
        return 0;
    }

    pub fn pop(&mut self) -> u8 {
        if self.is_empty() {
            panic!("the buffer is empty")
        }
        let index = 8 + (self.get_head() % self.capacity);
        self.set_head(&self.get_head() + 1);
        return self.buffer[index as usize];
    }

    pub fn is_full(&self) -> bool {
        return self.get_tail() - self.get_head() == self.capacity;
    }

    pub fn is_empty(&self) -> bool {
        return self.get_tail() == self.get_head();
    }

    pub fn peek(&self) -> u8 {
        // header offset + actual head
        let index = 8 + (self.get_head() % self.capacity);
        return self.buffer[index as usize];
    }

    pub fn get_tail(&self) -> u32 {
        return u32::from_be_bytes(self.buffer[4..8].try_into().unwrap());
    }

    pub fn set_tail(&mut self, tail: u32) {
        let dst: &mut [u8; 4] = (&mut self.buffer[4..8]).try_into().unwrap();
        *dst = tail.to_be_bytes();
    }

    pub fn get_head(&self) -> u32 {
        return u32::from_be_bytes(self.buffer[0..4].try_into().unwrap());
    }

    pub fn set_head(&mut self, head: u32) {
        let dst: &mut [u8; 4] = (&mut self.buffer[0..4]).try_into().unwrap();
        *dst = head.to_be_bytes();
    }
    //
}

impl Drop for RingBuffer<'_> {
    fn drop(&mut self) {
        unsafe { libc::close(self.shmid as i32) };
        let name = c"/ring6";
        let ret = unsafe { libc::shm_unlink(name.as_ptr()) };
        if ret != 0 {
            panic!("can't unlink the shared memory segment")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        // let result = add(2, 2);
        // assert_eq!(result, 4);
    }
}
