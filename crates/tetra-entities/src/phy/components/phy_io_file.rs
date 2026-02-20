use crossbeam_channel::{Sender, unbounded};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::thread;

#[derive(Debug, Clone)]
pub enum FileWriteMsg {
    WriteBlock(Vec<u8>),
    WriteHeaderAndBlock(u8, u64, Vec<u8>),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhyIoFileMode {
    Write,
    Read,
    ReadRepeat,
}

#[derive(Debug)]
pub enum PhyIoError {
    Io(String),
    Eof,
}

impl From<io::Error> for PhyIoError {
    fn from(err: io::Error) -> Self {
        PhyIoError::Io(err.to_string())
    }
}

pub struct PhyIoFile {
    file: File,
    mode: PhyIoFileMode,
    file_size: u64,
}

impl PhyIoFile {
    /// Create a new PhyIoFile instance
    ///
    /// # Arguments
    /// * `filename` - Path to the file
    /// * `mode` - Write, Read, or ReadRepeat mode
    pub fn new<P: AsRef<Path>>(filename: P, mode: PhyIoFileMode) -> io::Result<Self> {
        let file = match mode {
            PhyIoFileMode::Read | PhyIoFileMode::ReadRepeat => OpenOptions::new().read(true).open(&filename)?,
            PhyIoFileMode::Write => OpenOptions::new().write(true).create(true).truncate(true).open(&filename)?,
        };

        let file_size = if mode == PhyIoFileMode::Read { file.metadata()?.len() } else { 0 };

        Ok(Self { file, mode, file_size })
    }

    /// Read a block of data from the file
    ///
    /// # Arguments
    /// * `buffer` - Buffer to read data into (size determines block size)
    ///
    /// # Returns
    /// * `Ok(())` - Block successfully read
    /// * `Err(PhyIoError::Eof)` - EOF reached and eof_behavior is Stop
    /// * `Err(PhyIoError::Io)` - I/O error occurred
    pub fn read_block(&mut self, buffer: &mut [u8]) -> Result<(), PhyIoError> {
        let block_size = buffer.len();
        let mut bytes_read = 0;

        while bytes_read < block_size {
            match self.file.read(&mut buffer[bytes_read..]) {
                Ok(0) => {
                    // EOF reached
                    match self.mode {
                        PhyIoFileMode::Read => {
                            return Err(PhyIoError::Eof);
                        }
                        PhyIoFileMode::ReadRepeat => {
                            // Seek back to beginning and continue reading
                            self.file.seek(SeekFrom::Start(0))?;

                            // If we had a partial block, it means the file doesn't contain
                            // an integer number of blocks. In this case, discard the partial
                            // block and start fresh from the beginning.
                            if bytes_read > 0 {
                                bytes_read = 0;
                                tracing::debug!("Discarding partial block at EOF, repeating from start");
                            }
                        }
                        PhyIoFileMode::Write => {
                            panic!(); // never happens
                        }
                    }
                }
                Ok(n) => {
                    bytes_read += n;
                }
                Err(e) => {
                    return Err(PhyIoError::from(e));
                }
            }
        }

        Ok(())
    }

    pub fn write_header_and_block(&mut self, field_type: u8, timestamp: u64, data: &[u8]) -> Result<(), PhyIoError> {
        if self.mode != PhyIoFileMode::Write {
            return Err(PhyIoError::Io("File not opened for writing".to_string()));
        }

        self.file.write_all(&field_type.to_be_bytes())?;
        self.file.write_all(&timestamp.to_be_bytes())?;
        self.write_block(data)?;
        Ok(())
    }

    /// Write a block of data to the file
    ///
    /// # Arguments
    /// * `data` - Data to write
    ///
    /// # Returns
    /// * `Ok(())` - Block successfully written
    /// * `Err(PhyIoError::Io)` - I/O error occurred or file not opened for writing
    pub fn write_block(&mut self, data: &[u8]) -> Result<(), PhyIoError> {
        if self.mode != PhyIoFileMode::Write {
            return Err(PhyIoError::Io("File not opened for writing".to_string()));
        }

        self.file.write_all(data)?;
        Ok(())
    }

    /// Flush any buffered data to disk
    pub fn flush(&mut self) -> Result<(), PhyIoError> {
        self.file.flush()?;
        Ok(())
    }

    /// Get the current file position
    pub fn position(&mut self) -> io::Result<u64> {
        self.file.stream_position()
    }

    /// Get the file size (only meaningful for read mode)
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Seek to a specific position in the file
    pub fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }

    /// Create an async writer that spawns a background thread for file writes
    /// Returns a Sender that can be used to queue write operations
    pub fn create_async_writer<P: AsRef<Path>>(filename: P, thread_name: String) -> io::Result<Sender<FileWriteMsg>> {
        let file_path = filename.as_ref().to_path_buf();
        let (sender, receiver) = unbounded::<FileWriteMsg>();
        // let thread_name = format!("phy-io-writer-{}", file_path.display());

        thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                if let Ok(mut file) = PhyIoFile::new(&file_path, PhyIoFileMode::Write) {
                    while let Ok(msg) = receiver.recv() {
                        match msg {
                            FileWriteMsg::WriteBlock(data) => {
                                let _ = file.write_block(&data);
                            }
                            FileWriteMsg::WriteHeaderAndBlock(field_type, timestamp, data) => {
                                let _ = file.write_header_and_block(field_type, timestamp, &data);
                            }
                            FileWriteMsg::Shutdown => break,
                        }
                    }
                }
            })
            .expect("Failed to spawn phy-io-writer thread");

        Ok(sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::io::Write;

    fn create_temp_file(data: &[u8]) -> (String, std::path::PathBuf) {
        let mut path = env::temp_dir();
        let filename = format!(
            "phy_io_test_{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(filename.clone());

        let mut file = File::create(&path).unwrap();
        file.write_all(data).unwrap();
        file.flush().unwrap();

        (filename, path)
    }

    #[test]
    fn test_write_and_read_block() {
        let mut path = env::temp_dir();
        let filename = format!(
            "phy_io_test_write_{}.bin",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        path.push(&filename);

        // Write some data
        {
            let mut writer = PhyIoFile::new(&path, PhyIoFileMode::Write).unwrap();
            let data = [1u8, 2, 3, 4, 5, 6, 7, 8];
            writer.write_block(&data).unwrap();
            writer.flush().unwrap();
        }

        // Read it back
        {
            let mut reader = PhyIoFile::new(&path, PhyIoFileMode::Read).unwrap();
            let mut buffer = [0u8; 8];
            reader.read_block(&mut buffer).unwrap();
            assert_eq!(buffer, [1, 2, 3, 4, 5, 6, 7, 8]);
        }

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_eof_stop_behavior() {
        let (_filename, path) = create_temp_file(&[1u8, 2, 3, 4]);

        let mut reader = PhyIoFile::new(&path, PhyIoFileMode::Read).unwrap();
        let mut buffer = [0u8; 4];

        // First read should succeed
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4]);

        // Second read should hit EOF
        assert!(matches!(reader.read_block(&mut buffer), Err(PhyIoError::Eof)));

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_eof_loop_behavior() {
        let (_filename, path) = create_temp_file(&[1u8, 2, 3, 4]);

        let mut reader = PhyIoFile::new(&path, PhyIoFileMode::ReadRepeat).unwrap();
        let mut buffer = [0u8; 4];

        // First read
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4]);

        // Second read should loop back to beginning
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4]);

        // Third read should also work
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4]);

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_partial_block_loop() {
        let (_filename, path) = create_temp_file(&[1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let mut reader = PhyIoFile::new(&path, PhyIoFileMode::ReadRepeat).unwrap();
        let mut buffer = [0u8; 8];

        // First read gets first 8 bytes
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4, 5, 6, 7, 8]);

        // Second read should discard the 2-byte partial block and loop back
        assert!(reader.read_block(&mut buffer).is_ok());
        assert_eq!(buffer, [1, 2, 3, 4, 5, 6, 7, 8]);

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
}
