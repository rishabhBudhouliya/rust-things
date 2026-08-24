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

    // let's use the fs scan dir function
    // assume the dir only contains files that have data

    // goal 2: define get/set functions for the KeyValue struct
    // we should not promise an owner value, instead, it should be a peek/reference into the storage
    pub fn get(&self, key: String) -> Option<&u32> {
        dbg!(&self.map);
        return self.map.get(&key);
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
                0o666,
            ) as i32
        };
        let _ = unsafe { libc::write(fd as i32, ptr as *mut c_void, size_buffer as usize) };
        // separate the key value insertion from file write
        self.map.insert(key, value);
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
        let fd =
            unsafe { libc::open(c_filename.as_ptr(), libc::O_RDWR | libc::O_APPEND, 0o666) as i32 };
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
        if !prefix_key_name.is_empty() {
            key = prefix_key_name.clone() + "." + &key;
        }
        let value = str::from_utf8(key_value_iter.next().unwrap()).unwrap();
        map.insert(key, u32::from_str_radix(value, 10).unwrap());
    }
}

fn main() {
    let mut server = KeyValue::new(String::from("data/"));
    let ret = server
        .get(String::from("sample.rishabh2"))
        .expect("key not present in the map");
    println!("value from rishabh2: {}", ret);
    server.set(String::from("rishabh4"), 89);
    dbg!(server);
}
