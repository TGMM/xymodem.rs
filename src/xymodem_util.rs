use std::{
    fmt::Display,
    fmt::Formatter,
    io::{self, Read},
};

pub fn calc_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0, |x, &y| x.wrapping_add(y))
}

pub fn calc_crc(data: &[u8]) -> u16 {
    crc16::State::<crc16::XMODEM>::calculate(data)
}

pub fn get_byte<R: Read>(reader: &mut R) -> std::io::Result<u8> {
    let mut buff = [0];
    (reader.read_exact(&mut buff))?;
    Ok(buff[0])
}

/// Turns timeout errors into `Ok(None)`
pub fn get_byte_timeout<R: Read>(reader: &mut R) -> std::io::Result<Option<u8>> {
    match get_byte(reader) {
        Ok(c) => Ok(Some(c)),
        Err(err) => {
            if err.kind() == io::ErrorKind::TimedOut {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),

    /// The number of communications errors exceeded `max_errors` in a single
    /// transmission.
    ExhaustedRetries,

    /// The transmission was canceled by the other end of the channel.
    Canceled,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(io_err) => io_err.fmt(f),
            Error::ExhaustedRetries => write!(f, "Transfer retries exhuasted"),
            Error::Canceled => write!(f, "Transfer canceled"),
        }
    }
}

impl std::error::Error for Error {}
