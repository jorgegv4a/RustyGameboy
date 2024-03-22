use serde::{Deserialize};
use serde_json::{Result, Value};
use std::fs;

#[derive(Deserialize, Debug)]
pub struct Opcode {
    mnemonic: String,
    length: u8,
    cycles: Vec<u8>,

    #[serde(default)]
    operand1: Option<String>,
    #[serde(default)]
    operand2: Option<String>,
}


pub fn get_opcodes() -> Result<Value>{
    let contents = fs::read_to_string("opcodes.json").expect("Couldn't find or load that file.");
    serde_json::from_str(&contents)
}