extern crate serialport;
extern crate xymodem;

use xymodem::xmodem::{BlockLength, Checksum, Xmodem};

fn main() {
    let mut xmodem_sender = Xmodem::new();
    xmodem_sender.block_length = BlockLength::Standard;
    xmodem_sender.max_errors = 10;

    let mut recv_data = Vec::new();
    let mut serialport = serialport::new("COM3", 115200)
        .open()
        .expect("Failed to open port");

    println!("Current timeout: {:#?}", serialport.timeout());
    let _ = serialport.set_timeout(std::time::Duration::from_millis(100));
    println!("Starting test...");

    const MAX_INITIAL_ERRORS: u32 = u32::MAX;
    let mut initial_errors: u32 = 0;

    loop {
        match xmodem_sender.recv(
            &mut Box::new(serialport.as_mut()),
            &mut recv_data,
            Checksum::CRC16,
        ) {
            Ok(v) => {
                println!("Ahuevo");
                dbg!(v);
                break;
            }
            Err(err) => {
                match err {
                    xmodem::Error::ExhaustedRetries => {
                        initial_errors += 1;
                        if initial_errors >= MAX_INITIAL_ERRORS {
                            print!("Exceeded max count of retries");
                            std::process::exit(1);
                        }
                    }
                    _ => {
                        print!("Unknown error");
                        std::process::exit(1);
                    }
                }
                dbg!(err);
            }
        }
    }

    println!("Ending test...");
}
