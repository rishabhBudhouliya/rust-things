/*
 * Design a ring buffer backed by a shared memory and protected by a semaphore
 * Learn how to create and maintain a shared memory in std io
 * Learn how to create, link and unlink a mmap reference
 * (why do I need an mmap invocation to link the shared memory segment?)
 * Learn how to use a kernel backed semaphore
 */

import libc
const HEADER: u8 = 8;

/*
 *  shm_open — create/open the segment, returns fd
    ftruncate — size it (creator only)
    mmap — map it, MAP_SHARED
    close — fd, right after mapping
 */
struct RingBuffer {
    capacity: usize,
    shmid: usize,
    buffer: Vec<u8>,
    head: usize,
    tail: usize
}

impl RingBuffer {
    // allocate a shm and
    pub fn new() -> RingBuffer {
        let name = "/ring";
        shmid: unsafe {libc::shm_open(name, libc::O_RDWR | libc::CREAT, )}
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
