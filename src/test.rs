extern crate serialport;
extern crate ymodem;

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use ymodem::xmodem::*;
use ymodem::ymodem::*;

fn main() {
    ymodem_receive().unwrap();
}

fn ymodem_receive() -> std::io::Result<()> {
    let mut ymodem_sender = Ymodem::new();
    ymodem_sender.max_errors = 10;
    ymodem_sender.max_initial_errors = 10;

    let mut recv_data: Vec<u8> = Vec::new();
    let mut serialport = serialport::new("COM3", 115200)
        .open()
        .expect("Failed to open port");

    let _ = serialport.set_timeout(std::time::Duration::from_millis(500));
    println!("Current timeout: {:#?}", serialport.timeout());

    println!("Starting test...");

    let mut file_name = String::new();
    let mut file_size = 0;
    ymodem_sender
        .recv(
            &mut serialport,
            &mut recv_data,
            &mut file_name,
            &mut file_size,
        )
        .unwrap();

    println!("Attempting to write {}", file_name);

    let mut file = (std::fs::File::create(file_name))?;
    file.write_all(&mut recv_data)?;

    println!("Ending test...");

    Ok(())
}

fn xmodem_receive() -> std::io::Result<()> {
    let mut xmodem_sender = Xmodem::new();
    xmodem_sender.max_errors = 10;
    xmodem_sender.max_initial_errors = 10;
    xmodem_sender.block_length = BlockLength::OneK;

    let mut recv_data: Vec<u8> = Vec::new();
    let mut serialport = serialport::new("COM3", 115200)
        .open()
        .expect("Failed to open port");

    let _ = serialport.set_timeout(std::time::Duration::from_millis(500));
    println!("Current timeout: {:#?}", serialport.timeout());

    println!("Starting test...");

    xmodem_sender
        .recv(&mut serialport, &mut recv_data, Checksum::CRC16)
        .unwrap();

    let mut file = (std::fs::File::create("sample.stl"))?;
    file.write_all(&mut recv_data)?;

    println!("Ending test...");

    Ok(())
}

fn ymodem_send() {
    let mut ymodem_sender = Ymodem::new();
    ymodem_sender.max_errors = 10;
    ymodem_sender.max_initial_errors = 10;

    let mut serialport = serialport::new("COM2", 115200)
        .open()
        .expect("Failed to open port");

    let _ = serialport.set_timeout(std::time::Duration::from_millis(1000));
    println!("Current timeout: {:#?}", serialport.timeout());

    println!("Starting test...");

    let file_name = "firm.sfb";
    let mut file = std::fs::File::open("firm.sfb").unwrap();
    let file_size = file.metadata().unwrap().len();

    ymodem_sender
        .send(&mut serialport, &mut file, file_name.to_string(), file_size)
        .unwrap();

    println!("Attempting to write {}", file_name);

    println!("Ending test...");
}
