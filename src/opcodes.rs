use serde::{Deserialize};
use serde_json::{Result, Value};
use std::fs;

#[derive(Deserialize, Debug)]
pub struct Opcode {
    pub mnemonic: String,
    pub length: u8,
    pub cycles: Vec<u8>,
}


pub fn get_opcodes() -> Result<Value>{
    let contents = fs::read_to_string("opcodes.json").expect("Couldn't find or load that file.");
    serde_json::from_str(&contents)
}