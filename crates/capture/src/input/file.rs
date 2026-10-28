//! Binary event files of a recording (`cursor.bin`, `keys.bin`): an 8-byte header, then
//! fixed-size little-endian records in the order the events happened.
//!
//! Header: magic (4 bytes), format version (u16), record size in bytes (u16). Records are
//! appended while recording and flushed at least every [`FLUSH`]. A crash leaves the file
//! ending at the last flush, possibly inside a record; the reader drops that partial record.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::CaptureError;
use crate::event::HostTime;

/// How long written events may wait in memory: what a crash of the process loses at most.
pub const FLUSH: Duration = Duration::from_millis(100);

const HEADER: usize = 8;

/// An event at a host time (QPC, µs; tick 0 of the recording is the manifest's track start).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timed<E> {
    pub at: HostTime,
    pub event: E,
}

/// One record of an event file.
pub trait Record: Sized {
    const MAGIC: [u8; 4];
    const VERSION: u16;
    /// Bytes per record, including the 8-byte time.
    const SIZE: usize;

    /// Appends exactly [`Self::SIZE`] bytes.
    fn encode(&self, out: &mut Vec<u8>);

    /// Reads one record of exactly [`Self::SIZE`] bytes; the error says what is wrong.
    fn decode(bytes: &[u8]) -> Result<Self, String>;
}

/// Appends records to a new event file.
pub struct RecordWriter<R> {
    path: PathBuf,
    file: BufWriter<File>,
    bytes: Vec<u8>,
    record: PhantomData<R>,
}

impl<R: Record> RecordWriter<R> {
    /// Creates the file (an existing one is replaced) and writes the header.
    pub fn create(path: &Path) -> Result<Self, CaptureError> {
        let file = File::create(path).map_err(CaptureError::file("создать", path))?;
        let mut writer = Self {
            path: path.to_path_buf(),
            file: BufWriter::new(file),
            bytes: Vec::with_capacity(R::SIZE),
            record: PhantomData,
        };
        let mut header = R::MAGIC.to_vec();
        header.extend_from_slice(&R::VERSION.to_le_bytes());
        header.extend_from_slice(&(R::SIZE as u16).to_le_bytes());
        writer.put(&header)?;
        Ok(writer)
    }

    pub fn write(&mut self, record: &R) -> Result<(), CaptureError> {
        let mut bytes = std::mem::take(&mut self.bytes);
        bytes.clear();
        record.encode(&mut bytes);
        let written = self.put(&bytes);
        self.bytes = bytes;
        written
    }

    /// Hands the buffered records to the system.
    pub fn flush(&mut self) -> Result<(), CaptureError> {
        self.file
            .flush()
            .map_err(CaptureError::file("записать", &self.path))
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), CaptureError> {
        self.file
            .write_all(bytes)
            .map_err(CaptureError::file("записать", &self.path))
    }
}

/// Reads every whole record of an event file; a partial last record (crash) is dropped.
pub fn read_records<R: Record>(path: &Path) -> Result<Vec<R>, CaptureError> {
    let bytes = std::fs::read(path).map_err(CaptureError::file("прочитать", path))?;
    let bad = |message: String| CaptureError::BadTelemetry {
        path: path.to_path_buf(),
        message,
    };
    let header = bytes
        .get(..HEADER)
        .ok_or_else(|| bad("нет заголовка".to_owned()))?;
    if header[..4] != R::MAGIC {
        return Err(bad("это не файл событий DevCast".to_owned()));
    }
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version > R::VERSION {
        return Err(bad(format!(
            "версия {version} новее, чем умеет программа ({}): обнови программу",
            R::VERSION
        )));
    }
    let size = usize::from(u16::from_le_bytes([header[6], header[7]]));
    if size != R::SIZE {
        return Err(bad(format!("запись {size} байт вместо {}", R::SIZE)));
    }
    bytes[HEADER..]
        .chunks_exact(R::SIZE)
        .map(|record| R::decode(record).map_err(bad))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Number(u32);

    impl Record for Number {
        const MAGIC: [u8; 4] = *b"TEST";
        const VERSION: u16 = 2;
        const SIZE: usize = 4;

        fn encode(&self, out: &mut Vec<u8>) {
            out.extend_from_slice(&self.0.to_le_bytes());
        }

        fn decode(bytes: &[u8]) -> Result<Self, String> {
            match bytes {
                [0, 0, 0, 0] => Err("ноль".to_owned()),
                [a, b, c, d] => Ok(Self(u32::from_le_bytes([*a, *b, *c, *d]))),
                _ => Err("не 4 байта".to_owned()),
            }
        }
    }

    #[test]
    fn records_round_trip_and_partial_tail_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.bin");
        let mut writer = RecordWriter::<Number>::create(&path).unwrap();
        for n in 1..=3 {
            writer.write(&Number(n)).unwrap();
        }
        writer.flush().unwrap();
        drop(writer);
        let mut bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"TEST\x02\x00\x04\x00");
        assert_eq!(
            read_records::<Number>(&path).unwrap(),
            [1, 2, 3].map(Number)
        );
        bytes.extend_from_slice(&[9, 9]);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            read_records::<Number>(&path).unwrap(),
            [1, 2, 3].map(Number)
        );
    }

    #[test]
    fn foreign_newer_or_broken_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.bin");
        let cases: [(&[u8], &str); 5] = [
            (b"TES", "нет заголовка"),
            (b"NOPE\x01\x00\x04\x00", "не файл событий"),
            (b"TEST\x03\x00\x04\x00", "обнови программу"),
            (b"TEST\x01\x00\x08\x00", "8 байт вместо 4"),
            (b"TEST\x01\x00\x04\x00\x00\x00\x00\x00", "ноль"),
        ];
        for (bytes, message) in cases {
            std::fs::write(&path, bytes).unwrap();
            let error = read_records::<Number>(&path).unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
