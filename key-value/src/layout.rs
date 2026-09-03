/*
* WireFormat {
    length,
    Op,
    value,
    key,
}
*/

use std::str::FromStr;

#[repr(u8)]
#[derive(Debug)]
pub enum Op {
    GET = 1,
    SET = 2,
    DELETE = 3,
}

#[derive(Debug)]
pub enum Request<'a> {
    GET { key: &'a str },
    SET { key: &'a str, value: u32 },
    DELETE { key: &'a str },
}

#[repr(u8)]
#[derive(Debug)]
pub enum Status {
    Success = 0,
    Failure = 1,
}

#[derive(Debug)]
pub enum Response {
    GetResponse { status: Status, value: u32 },
    SetResponse { status: Status },
    DeleteResponse { status: Status, value: bool },
}

impl TryFrom<u8> for Op {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Op::GET),
            2 => Ok(Op::SET),
            3 => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {value}")),
        }
    }
}

impl FromStr for Op {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Op::GET),
            "SET" => Ok(Op::SET),
            "DELETE" => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {s}")),
        }
    }
}

impl TryFrom<&str> for Op {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            get => Ok(Op::GET),
            set => Ok(Op::SET),
            delete => Ok(Op::DELETE),
            _ => Err(format!("unknown opcode {value}")),
        }
    }
}

pub fn parse_request(buffer: &Vec<u8>) -> Request {
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

pub fn parse_response(response: Response) -> Vec<u8> {
    let mut result = Vec::new();
    match response {
        Response::GetResponse { status, value } => {
            result.push(Op::GET as u8);
            result.push(status as u8);
            result.extend_from_slice(&value.to_be_bytes());
        }
        Response::DeleteResponse { status, value } => {
            result.push(Op::DELETE as u8);
            result.push(status as u8);
            result.push(value as u8);
        }
        Response::SetResponse { status } => {
            result.push(Op::SET as u8);
            result.push(status as u8);
        }
    };
    result
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
        key: (key.unwrap()),
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

pub fn frame_encoder(request: Request) -> Vec<u8> {
    let mut result = Vec::new();
    let (op, key, value) = match request {
        Request::GET { key } => (Op::GET, key, Option::None),
        Request::SET { key, value } => (Op::SET, key, Option::Some(value)),
        Request::DELETE { key } => (Op::DELETE, key, Option::None),
        _ => panic!(""),
    };
    // encode the length first
    result.extend_from_slice(&(key.len() as u32).to_be_bytes());
    // op code
    result.push(op as u8);
    // value if available
    if value.is_some() {
        result.extend_from_slice(&value.unwrap().to_be_bytes());
    }
    // key now
    result.extend_from_slice(&key.as_bytes());
    result
}
