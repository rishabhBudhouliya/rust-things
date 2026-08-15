use ring_buffer::RingBuffer;
fn main() {
    let mut rb = RingBuffer::new(10).expect("allocation failed");
    for i in 0..12 {
        println!("the head is: {}", &rb.get_head());
        println!("the tail is: {}", &rb.get_tail());
        dbg!(&rb);
        let ret = &rb.push(i as u8);
    }
    rb.close();
    assert_eq!(*(&rb.peek()), 9);
    println!("{}", &rb.peek());
}
