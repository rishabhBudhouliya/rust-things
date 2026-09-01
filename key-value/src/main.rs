/*
 * Build a key value server with incremental goals following the cs664
 */

use libc;
use std::collections::HashMap;
use std::ffi::CString;
use std::ffi::c_void;
use std::fs::read_dir;
use std::io;
use std::mem::MaybeUninit;
use std::str::FromStr;

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
    SET { key: String, value: u32 },
    DELETE { key: &'a str },
}

impl TryFrom<u8> for Op {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Op::GET),
            2 => Ok(Op::SET),
            3 => Ok(Op::DELETE),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
struct KeyValue {
    // mapping of filename to fd
    dir: String,
    file_map: HashMap<String, i32>,
    map: HashMap<String, u32>,
}
// the map is keyed by the full dotted key. The filename is derived from the key on demand, never stored as part of it.
// the job of init is to: 1) initialize file map that has fds and keyname
// 2) initialize the internal hashmap
impl KeyValue {
    pub fn new(dir_name: String) -> Self {
        let file_map = scan_files(&dir_name);
        // now we need to construct an internal hashmap with actual keys and value
        let map = parse_map(&file_map);
        KeyValue {
            dir: dir_name,
            file_map,
            map,
        }
    }

    pub fn listen(&self) {
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
        }

        // start listening, keep the queue for backlog 0
        let ret = unsafe { libc::listen(socket_fd, 0) };
        if ret == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            println!("unable to start listening to socket");
        }
        // accept() blocks the caller until a connection is present unless non blocking sockets are used
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
            println!("received no data from the client")
        } else {
            println!(
                "received some data: {}",
                str::from_utf8(&listen_buff[0..50]).unwrap()
            )
        }
        let request = parse_request(&listen_buff);
        println!("{request:?}")
    }

    // let's use the fs scan dir function
    // assume the dir only contains files that have data

    // goal 2: define get/set functions for the KeyValue struct
    // we should not promise an owner value, instead, it should be a peek/reference into the storage
    pub fn get(&self, key: String) -> Option<&u32> {
        return self.map.get(&key);
    }

    // list all keys
    pub fn get_all(&self) -> Vec<&String> {
        self.map.keys().collect()
    }

    // delete a key and confirm if that happened
    pub fn delete_key(&mut self, deletion_key: &str) -> bool {
        //file io to remove stuff
        let t = deletion_key.to_string();
        let components = split_key(&t);
        let mut derived_key = "";
        let mut filename = "";
        let mut stored = String::new();
        if let Some(prefix_key) = components {
            filename = prefix_key.0;
            derived_key = prefix_key.1;
            stored = format!("{}:{}\n", derived_key, u32::MAX);
        } else {
            filename = "default";
            // derived_key = format!("{}.{}", filename, key);
            stored = format!("{}:{}\n", deletion_key, u32::MAX);
        };
        let written_buffer = stored.into_bytes();
        let size_buffer = written_buffer.len();
        let ptr: *const u8 = written_buffer.as_ptr();

        let c_filename =
            CString::new(self.dir.clone() + filename).expect("Failed to create CString");
        let fd = unsafe { libc::open(c_filename.as_ptr(), libc::O_RDWR | libc::O_APPEND) as i32 };
        let _ = unsafe { libc::write(fd as i32, ptr as *mut c_void, size_buffer as usize) };
        self.map.remove(deletion_key);
        true
    }

    // set requires two steps
    // 1) mutate the in-memory hashmap
    // 2) mutate the on-disk hashmap
    // implement deduplication
    pub fn set(&mut self, key: String, value: u32) {
        if self.map.get(&key) == Some(&value) {
            println!("Value already exists");
            return;
        }
        let components = split_key(&key);
        let mut derived_key = "";
        let mut filename = "";
        let mut stored = String::new();
        if let Some(prefix_key) = components {
            filename = prefix_key.0;
            derived_key = prefix_key.1;
            stored = format!("{}:{}\n", derived_key, value);
        } else {
            filename = "default";
            // derived_key = format!("{}.{}", filename, key);
            stored = format!("{}:{}\n", key, value);
        };
        let written_buffer = stored.into_bytes();
        let size_buffer = written_buffer.len();
        let ptr: *const u8 = written_buffer.as_ptr();

        let c_filename =
            CString::new(self.dir.clone() + filename).expect("Failed to create CString");
        let fd = unsafe {
            libc::open(
                c_filename.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_APPEND,
                0o600,
            ) as i32
        };
        let _ = unsafe { libc::write(fd as i32, ptr as *mut c_void, size_buffer as usize) };
        // separate the key value insertion from file write
        self.map.insert(key, value);
    }
}
// this takes in the byte stream and checks the op to figure out what to do with the request
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
        key: (String::from_str(key.unwrap()).unwrap()),
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

// constructs the file name to fd mapping for the kv server
fn scan_files(dir_name: &String) -> HashMap<String, i32> {
    let itr = read_dir(dir_name).expect("unable to read dir");
    let mut file_map = HashMap::new();
    for item in itr {
        let item = item.expect("unable to visit an item inside dir");
        let mut path = item.path();
        if path.is_dir() {
            continue;
        }
        let file_path = path
            .into_os_string()
            .into_string()
            .expect("unable to translate path to string");
        path = item.path();
        let file_name: &str = path
            .file_stem()
            .expect("unable to parse filename")
            .to_str()
            .unwrap();
        let c_filename = CString::new(file_path.clone()).expect("Failed to create CString");
        let fd = unsafe { libc::open(c_filename.as_ptr(), libc::O_RDWR | libc::O_APPEND) as i32 };
        if fd == -1 {
            let err = io::Error::last_os_error();
            dbg!(err);
            println!("unable to open file");
            continue;
        }
        file_map.insert(String::from_str(file_name).unwrap(), fd);
    }
    file_map
}

fn parse_map(file_map: &HashMap<String, i32>) -> HashMap<String, u32> {
    let mut map: HashMap<String, u32> = HashMap::new();
    for (name, fd) in file_map {
        parse_file(&mut map, name, fd);
    }
    return map;
}

fn parse_file(map: &mut HashMap<String, u32>, prefix_key_name: &String, fd: &i32) {
    let mut stat: MaybeUninit<libc::stat> = MaybeUninit::uninit();
    let ret = unsafe { libc::fstat(fd.clone(), stat.as_mut_ptr()) };
    let size = if ret != -1 {
        unsafe { stat.assume_init() }.st_size
    } else {
        0
    };
    let buffer = vec![0; size as usize];
    let buffer_start: *const u8 = buffer.as_ptr();
    let read_bytes = unsafe { libc::read(fd.clone(), buffer_start as *mut c_void, size as usize) };
    if read_bytes == 0 {
        panic!("Couldn't read anything");
    }
    let iter = buffer.split(|num| *num == b'\n');
    for record in iter {
        if record.is_empty() {
            println!("Encountered an empty record");
            break;
        }
        let mut key_value_iter = record.split(|num| *num == b':');
        // add a check on whether the iter has length = 2 or not
        let mut key =
            String::from_utf8((key_value_iter.next().unwrap()).try_into().unwrap()).unwrap();
        if !prefix_key_name.is_empty() && prefix_key_name != "default" {
            key = prefix_key_name.clone() + "." + &key;
        }
        let value = str::from_utf8(key_value_iter.next().unwrap()).unwrap();
        if map.get(&key).is_some() {
            println!("Value already exists");
            continue;
        }
        map.insert(key, u32::from_str_radix(value, 10).unwrap());
    }
}

fn main() {
    let mut server = KeyValue::new(String::from("data/"));
    match server.get(String::from("sample.rishabh2")) {
        Some(key) => println!("value from rishabh2: {}", key),
        None => {}
    }
    server.set(String::from("rishabh4"), 89);
    let all_keys = server.get_all();

    server.listen();
    dbg!(all_keys);
    // server.delete_key("rishabh4");
    dbg!(server);
}
