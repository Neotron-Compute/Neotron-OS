//! Filesystem related types

use chrono::{Datelike, Timelike};
use embedded_sdmmc::RawVolume;

use crate::{bios, refcell::CsRefCell, API, FILESYSTEM};

/// Represents a block device that reads/writes disk blocks using the BIOS.
///
/// Currently only block device 0 is supported.
pub struct BiosBlock();

impl embedded_sdmmc::BlockDevice for BiosBlock {
    type Error = bios::Error;

    fn read(
        &self,
        blocks: &mut [embedded_sdmmc::Block],
        start_block_idx: embedded_sdmmc::BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        let api = API.get();
        let byte_slice = unsafe {
            core::slice::from_raw_parts_mut(
                blocks.as_mut_ptr() as *mut u8,
                blocks.len() * embedded_sdmmc::Block::LEN,
            )
        };
        match (api.block_read)(
            0,
            bios::block_dev::BlockIdx(u64::from(start_block_idx.0)),
            blocks.len() as u8,
            bios::FfiBuffer::new(byte_slice),
        ) {
            bios::ApiResult::Ok(_) => Ok(()),
            bios::ApiResult::Err(e) => Err(e),
        }
    }

    fn write(
        &self,
        blocks: &[embedded_sdmmc::Block],
        start_block_idx: embedded_sdmmc::BlockIdx,
    ) -> Result<(), Self::Error> {
        let api = API.get();
        let byte_slice = unsafe {
            core::slice::from_raw_parts(
                blocks.as_ptr() as *const u8,
                blocks.len() * embedded_sdmmc::Block::LEN,
            )
        };
        match (api.block_write)(
            0,
            bios::block_dev::BlockIdx(u64::from(start_block_idx.0)),
            blocks.len() as u8,
            bios::FfiByteSlice::new(byte_slice),
        ) {
            bios::ApiResult::Ok(_) => Ok(()),
            bios::ApiResult::Err(e) => Err(e),
        }
    }

    fn num_blocks(&self) -> Result<embedded_sdmmc::BlockCount, Self::Error> {
        let api = API.get();
        match (api.block_dev_get_info)(0) {
            bios::FfiOption::Some(info) => Ok(embedded_sdmmc::BlockCount(info.num_blocks as u32)),
            bios::FfiOption::None => Err(bios::Error::InvalidDevice),
        }
    }
}

/// A type that lets you fetch the current time from the BIOS.
pub struct BiosTime();

impl embedded_sdmmc::TimeSource for BiosTime {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        let time = API.get_time();
        embedded_sdmmc::Timestamp {
            year_since_1970: (time.year() - 1970) as u8,
            zero_indexed_month: time.month0() as u8,
            zero_indexed_day: time.day0() as u8,
            hours: time.hour() as u8,
            minutes: time.minute() as u8,
            seconds: time.second() as u8,
        }
    }
}

/// The errors this module can produce
#[derive(Debug)]
pub enum Error {
    /// Filesystem error
    Io(embedded_sdmmc::Error<bios::Error>),
}

impl From<embedded_sdmmc::Error<bios::Error>> for Error {
    fn from(value: embedded_sdmmc::Error<bios::Error>) -> Self {
        Error::Io(value)
    }
}

/// Represents an open file
pub struct File {
    inner: embedded_sdmmc::RawFile,
}

impl File {
    /// Read from a file
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        FILESYSTEM.file_read(self, buffer)
    }

    /// Write to a file
    pub fn write(&self, buffer: &[u8]) -> Result<(), Error> {
        FILESYSTEM.file_write(self, buffer)
    }

    /// Are we at the end of the file
    pub fn is_eof(&self) -> bool {
        FILESYSTEM
            .file_eof(self)
            .expect("File handle should be valid")
    }

    /// Seek to a position relative to the start of the file
    pub fn seek_from_start(&self, offset: u32) -> Result<(), Error> {
        FILESYSTEM.file_seek_from_start(self, offset)
    }

    /// What is the length of this file?
    pub fn length(&self) -> u32 {
        FILESYSTEM
            .file_length(self)
            .expect("File handle should be valid")
    }
}

impl Drop for File {
    fn drop(&mut self) {
        FILESYSTEM
            .close_raw_file(self.inner)
            .expect("Should only be dropping valid files!");
    }
}

/// Represent all open files and filesystems
pub struct Filesystem {
    volume_manager: CsRefCell<Option<embedded_sdmmc::VolumeManager<BiosBlock, BiosTime, 4, 4, 1>>>,
    first_volume: CsRefCell<Option<RawVolume>>,
}

impl Filesystem {
    /// Create a new filesystem
    pub const fn new() -> Filesystem {
        Filesystem {
            volume_manager: CsRefCell::new(None),
            first_volume: CsRefCell::new(None),
        }
    }

    /// Open a file on the filesystem
    pub fn open_file(&self, name: &str, mode: embedded_sdmmc::Mode) -> Result<File, Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        let mut volume = self.first_volume.lock();
        if volume.is_none() {
            *volume = Some(fs.open_raw_volume(embedded_sdmmc::VolumeIdx(0))?);
        }
        let volume = volume.unwrap();
        let mut root = fs.open_root_dir(volume)?.to_directory(fs);
        let file = root.open_file_in_dir(name, mode)?;
        let raw_file = file.to_raw_file();
        Ok(File { inner: raw_file })
    }

    /// Walk through the root directory
    pub fn iterate_root_dir<F>(&self, f: F) -> Result<(), Error>
    where
        F: FnMut(&embedded_sdmmc::DirEntry),
    {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        let mut volume = self.first_volume.lock();
        if volume.is_none() {
            *volume = Some(fs.open_raw_volume(embedded_sdmmc::VolumeIdx(0))?);
        }
        let volume = volume.unwrap();
        let mut root = fs.open_root_dir(volume)?.to_directory(fs);
        root.iterate_dir(f)?;
        Ok(())
    }

    /// Read from an open file
    pub fn file_read(&self, file: &File, buffer: &mut [u8]) -> Result<usize, Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        let bytes_read = fs.read(file.inner, buffer)?;
        Ok(bytes_read)
    }

    /// Write to an open file
    pub fn file_write(&self, file: &File, buffer: &[u8]) -> Result<(), Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        fs.write(file.inner, buffer)?;
        Ok(())
    }

    /// How large is a file?
    pub fn file_length(&self, file: &File) -> Result<u32, Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        let length = fs.file_length(file.inner)?;
        Ok(length)
    }

    /// Seek a file with an offset from the start of the file.
    pub fn file_seek_from_start(&self, file: &File, offset: u32) -> Result<(), Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        fs.file_seek_from_start(file.inner, offset)?;
        Ok(())
    }

    /// Are we at the end of the file
    pub fn file_eof(&self, file: &File) -> Result<bool, Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        let is_eof = fs.file_eof(file.inner)?;
        Ok(is_eof)
    }

    /// Close an open file
    ///
    /// Only used by File's drop impl.
    fn close_raw_file(&self, file: embedded_sdmmc::RawFile) -> Result<(), Error> {
        let mut fs = self.volume_manager.lock();
        if fs.is_none() {
            *fs = Some(embedded_sdmmc::VolumeManager::new(BiosBlock(), BiosTime()));
        }
        let fs = fs.as_mut().unwrap();
        fs.close_file(file)?;
        Ok(())
    }
}

// End of file
