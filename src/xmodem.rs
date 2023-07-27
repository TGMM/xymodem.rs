use std::io::{Read, Write};
pub use xymodem_util::*;

// TODO: Send CAN byte after too many errors
// TODO: Handle CAN bytes while sending

const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;
const CRC: u8 = 0x43;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Copy, Clone, Debug)]
pub enum Checksum {
    Standard,
    CRC16,
}

#[derive(Copy, Clone, Debug)]
pub enum BlockLength {
    Standard = 128,
    OneK = 1024,
}

/// Configuration for the XMODEM transfer.
#[derive(Copy, Clone, Debug)]
pub struct Xmodem {
    /// The number of errors that can occur before the communication is
    /// considered a failure. Errors include unexpected bytes and timeouts waiting for bytes.
    pub max_errors: u32,

    /// The number of errors that can occur before the communication is
    /// considered a failure. Errors include unexpected bytes and timeouts waiting for bytes.
    ///
    /// This only applies to the initial packet
    pub max_initial_errors: u32,

    /// The byte used to pad the last block. XMODEM can only send blocks of a certain size,
    /// so if the message is not a multiple of that size the last block needs to be padded.
    pub pad_byte: u8,

    /// The length of each block. There are only two options: 128-byte blocks (standard
    ///  XMODEM) or 1024-byte blocks (XMODEM-1k).
    pub block_length: BlockLength,

    /// The checksum mode used by XMODEM. This is determined by the receiver.
    checksum_mode: Checksum,
    errors: u32,
    initial_errors: u32,
}

impl Xmodem {
    /// Creates the XMODEM config with default parameters.
    pub fn new() -> Self {
        Xmodem {
            max_errors: 16,
            max_initial_errors: 16,
            pad_byte: 0x1a,
            block_length: BlockLength::Standard,
            checksum_mode: Checksum::Standard,
            errors: 0,
            initial_errors: 0,
        }
    }

    /// Starts the XMODEM transmission.
    ///
    /// `dev` should be the serial communication channel (e.g. the serial device).
    /// `stream` should be the message to send (e.g. a file).
    ///
    /// # Timeouts
    /// This method has no way of setting the timeout of `dev`, so it's up to the caller
    /// to set the timeout of the device before calling this method. Timeouts on receiving
    /// bytes will be counted against `max_errors`, but timeouts on transmitting bytes
    /// will be considered a fatal error.
    pub fn send<D: Read + Write, R: Read>(&mut self, dev: &mut D, stream: &mut R) -> Result<()> {
        self.errors = 0;

        debug!("Starting XMODEM transfer");
        (self.start_send(dev))?;
        debug!("First byte received. Sending stream.");
        (self.send_stream(dev, stream))?;
        debug!("Sending EOT");
        (self.finish_send(dev))?;

        Ok(())
    }

    /// Receive an XMODEM transmission.
    ///
    /// `dev` should be the serial communication channel (e.g. the serial device).
    /// The received data will be written to `outstream`.
    /// `checksum` indicates which checksum mode should be used; Checksum::Standard is
    /// a reasonable default.
    ///
    /// # Timeouts
    /// This method has no way of setting the timeout of `dev`, so it's up to the caller
    /// to set the timeout of the device before calling this method. Timeouts on receiving
    /// bytes will be counted against `max_errors`, but timeouts on transmitting bytes
    /// will be considered a fatal error.
    pub fn recv<D: Read + Write, W: Write>(
        &mut self,
        dev: &mut D,
        outstream: &mut W,
        checksum: Checksum,
    ) -> Result<()> {
        self.errors = 0;
        self.checksum_mode = checksum;
        let mut handled_first_packet = false;
        debug!("Starting XMODEM receive");

        let first_char;
        loop {
            (dev.write(&[match self.checksum_mode {
                Checksum::Standard => NAK,
                Checksum::CRC16 => CRC,
            }])?);

            match get_byte_timeout(dev)? {
                bt @ Some(SOH) | bt @ Some(STX) => {
                    // The first SOH or STX is used to initialize the transfer
                    first_char = bt.unwrap();
                    break;
                }
                _ => {
                    self.initial_errors += 1;
                    if self.initial_errors > self.max_initial_errors {
                        eprint!(
                            "Exhausted max retries ({}) while waiting for SOH or STX",
                            self.max_initial_errors
                        );
                        return Err(Error::ExhaustedRetries);
                    }
                }
            }
        }
        debug!("NCG sent. Receiving stream.");
        let mut packet_num: u8 = 1;
        loop {
            match if handled_first_packet {
                get_byte_timeout(dev)?
            } else {
                Some(first_char)
            } {
                bt @ Some(SOH) | bt @ Some(STX) => {
                    handled_first_packet = true;
                    // Handle next packet
                    let packet_size = match bt {
                        Some(SOH) => 128,
                        Some(STX) => 1024,
                        _ => 0, // Why does the compiler need this?
                    };
                    let pnum = (get_byte(dev))?; // specified packet number
                    let pnum_1c = (get_byte(dev))?; // same, 1's complemented
                                                    // We'll respond with cancel later if the packet number is wrong
                    let cancel_packet = packet_num != pnum || (255 - pnum) != pnum_1c;
                    let mut data: Vec<u8> = Vec::new();
                    data.resize(packet_size, 0);
                    (dev.read_exact(&mut data))?;
                    let success = match self.checksum_mode {
                        Checksum::Standard => {
                            let recv_checksum = (get_byte(dev))?;
                            calc_checksum(&data) == recv_checksum
                        }
                        Checksum::CRC16 => {
                            let recv_checksum =
                                (((get_byte(dev))? as u16) << 8) + (get_byte(dev))? as u16;
                            calc_crc(&data) == recv_checksum
                        }
                    };

                    if cancel_packet {
                        (dev.write(&[CAN]))?;
                        (dev.write(&[CAN]))?;
                        return Err(Error::Canceled);
                    }
                    if success {
                        packet_num = packet_num.wrapping_add(1);
                        (dev.write(&[ACK]))?;
                        (outstream.write_all(&data))?;
                    } else {
                        (dev.write(&[NAK]))?;
                        self.errors += 1;
                    }
                }
                Some(EOT) => {
                    // End of file
                    (dev.write(&[ACK]))?;
                    break;
                }
                Some(_) => {
                    warn!("Unrecognized symbol!");
                }
                None => {
                    if !handled_first_packet {
                        self.errors = self.max_errors;
                    } else {
                        self.errors += 1;
                    }
                    warn!("Timeout!")
                }
            }
            if self.errors >= self.max_errors {
                eprint!(
                    "Exhausted max retries ({}) while waiting for ACK for EOT",
                    self.max_errors
                );
                return Err(Error::ExhaustedRetries);
            }
        }
        Ok(())
    }
    fn start_send<D: Read + Write>(&mut self, dev: &mut D) -> Result<()> {
        let mut cancels = 0u32;
        loop {
            match (get_byte_timeout(dev))? {
                Some(c) => match c {
                    NAK => {
                        debug!("Standard checksum requested");
                        self.checksum_mode = Checksum::Standard;
                        return Ok(());
                    }
                    CRC => {
                        debug!("16-bit CRC requested");
                        self.checksum_mode = Checksum::CRC16;
                        return Ok(());
                    }
                    CAN => {
                        warn!("Cancel (CAN) byte received");
                        cancels += 1;
                    }
                    c => warn!("Unknown byte received at start of XMODEM transfer: {}", c),
                },
                None => warn!("Timed out waiting for start of XMODEM transfer."),
            }

            self.errors += 1;

            if cancels >= 2 {
                eprint!(
                    "Transmission canceled: received two cancel (CAN) bytes \
                        at start of XMODEM transfer"
                );
                return Err(Error::Canceled);
            }

            if self.errors >= self.max_errors {
                eprint!(
                    "Exhausted max retries ({}) at start of XMODEM transfer.",
                    self.max_errors
                );
                if let Err(err) = dev.write_all(&[CAN]) {
                    warn!("Error sending CAN byte: {}", err);
                }
                return Err(Error::ExhaustedRetries);
            }
        }
    }

    fn send_stream<D: Read + Write, R: Read>(&mut self, dev: &mut D, stream: &mut R) -> Result<()> {
        let mut block_num = 0u32;
        loop {
            let mut buff = vec![self.pad_byte; self.block_length as usize + 3];
            let n = (stream.read(&mut buff[3..]))?;
            if n == 0 {
                debug!("Reached EOF");
                return Ok(());
            }

            block_num += 1;
            buff[0] = match self.block_length {
                BlockLength::Standard => SOH,
                BlockLength::OneK => STX,
            };
            buff[1] = (block_num & 0xFF) as u8;
            buff[2] = 0xFF - buff[1];

            match self.checksum_mode {
                Checksum::Standard => {
                    let checksum = calc_checksum(&buff[3..]);
                    buff.push(checksum);
                }
                Checksum::CRC16 => {
                    let crc = calc_crc(&buff[3..]);
                    buff.push(((crc >> 8) & 0xFF) as u8);
                    buff.push((crc & 0xFF) as u8);
                }
            }

            debug!("Sending block {}", block_num);
            (dev.write_all(&buff))?;

            match (get_byte_timeout(dev))? {
                Some(c) => {
                    if c == ACK {
                        debug!("Received ACK for block {}", block_num);
                        continue;
                    } else {
                        warn!("Expected ACK, got {}", c);
                    }
                    // TODO handle CAN bytes
                }
                None => warn!("Timeout waiting for ACK for block {}", block_num),
            }

            self.errors += 1;

            if self.errors >= self.max_errors {
                eprint!(
                    "Exhausted max retries ({}) while sending block {} in XMODEM transfer",
                    self.max_errors, block_num
                );
                return Err(Error::ExhaustedRetries);
            }
        }
    }

    fn finish_send<D: Read + Write>(&mut self, dev: &mut D) -> Result<()> {
        loop {
            (dev.write_all(&[EOT]))?;

            match (get_byte_timeout(dev))? {
                Some(c) => {
                    if c == ACK {
                        info!("XMODEM transmission successful");
                        return Ok(());
                    } else {
                        warn!("Expected ACK, got {}", c);
                    }
                }
                None => warn!("Timeout waiting for ACK for EOT"),
            }

            self.errors += 1;

            if self.errors >= self.max_errors {
                eprint!(
                    "Exhausted max retries ({}) while waiting for ACK for EOT",
                    self.max_errors
                );
                return Err(Error::ExhaustedRetries);
            }
        }
    }
}
