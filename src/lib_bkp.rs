/*
 * Implementing a lock free ring buffer
 * 1) create a data structure that is naive, uses a memory backed vector and pointer arithmetic
 * 2) keep reading the blog, use memory transumtation with shared memory to make life easier
 * 3) look at atomics at Rust and multi threaded
 *
 */
#[derive(Debug)]
pub struct RingBuffer {
    //  bunch of bytes
    data: Vec<u8>,
    // apparently, 64 bytes is good for cache coherency
    pub head: u32,
    pub tail: u32,
    capacity: usize,
}

// now, I need a way to initialize this data structure, let's keep it on a heap for now and then define the contract to interact
// with it

// concerns in impl-aug-25.txt

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        if capacity > u32::MAX as usize {
            panic!("can construct larger than u32 max")
        }
        RingBuffer {
            data: vec![0; capacity],
            head: 0,
            tail: 0,
            capacity,
        }
    }

    fn is_full(&self) -> bool {
        (self.tail - self.head) == (self.capacity as u32)
    }

    fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    // what can we pop?
    pub fn peek(&self) -> u32 {
        if self.is_empty() {
            println!("sorry there's nothing to peek");
            return 0;
        }
        let index = (self.head as usize) % self.capacity;
        dbg!(index);
        let mut result = [0; 4];
        let mut counter = 0;
        for ifx in index..index + 4 {
            result[counter] = self.data[ifx % self.capacity];
            counter += 1;
        }
        dbg!(result);
        return u32::from_be_bytes(result);
    }

    pub fn pop(&mut self) -> u32 {
        if self.is_empty() {
            println!("sorry there's nothing to pop");
            return 0;
        }
        let index = ((self.head as usize) % self.capacity);
        let mut result = [0; 4];
        let mut counter = 0;
        for ifx in (index..index + 4) {
            result[counter] = self.data[ifx % self.capacity];
            counter += 1;
        }
        self.head += 4;
        return u32::from_be_bytes(result);
    }

    pub fn push(&mut self, value: u32) {
        if self.is_full() {
            println!("sorry no space left");
            return;
        }
        let index = (self.tail as usize);
        let result = u32::to_be_bytes(value);
        let mut counter = 0;
        for idx in (index..index + 4) {
            self.data[idx % self.capacity] = result[counter];
            counter += 1;
        }
        self.tail += 4
    }
}
