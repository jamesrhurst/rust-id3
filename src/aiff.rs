use crate::{Error, ErrorKind};
use crate::{Tag, Version};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, SeekFrom};
use std::path::Path;

pub fn load_aiff_id3(path: impl AsRef<Path>) -> crate::Result<Tag> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    loop {
        // Read chunk ID
        let mut chunk_id: [u8; 4] = [0; 4];
        // EOF
        if reader.read(&mut chunk_id)? == 0 {
            break;
        }

        // Read chunk size
        let mut chunk_size_raw: [u8; 4] = [0; 4];
        reader.read_exact(&mut chunk_size_raw)?;
        let chunk_size = u32::from_be_bytes(chunk_size_raw);

        // Skip FORM chunk type, get its chunks
        if &chunk_id == b"FORM" {
            reader.seek(SeekFrom::Current(4))?;
            continue;
        }

        if &chunk_id[0..3] == b"ID3" {
            return Ok(Tag::read_from(reader.take(chunk_size as u64))?);
        }

        reader.seek(SeekFrom::Current(chunk_size as i64))?;
    }

    Err(Error::new(ErrorKind::NoTag, "No tag chunk found!"))
}

// Wrapper to delete temp file
pub fn overwrite_aiff_id3(
    path: impl AsRef<Path>,
    tag: &Tag,
    version: Version,
) -> crate::Result<()> {
    let res = overwrite_aiff_id3_raw(&path, tag, version);
    if res.is_err() {
        let new_path = path.as_ref().with_extension("ID3TMP");
        // Ignore error as the file might be missing / not important.
        fs::remove_file(new_path).ok();
        return res;
    }

    Ok(())
}

fn overwrite_aiff_id3_raw(
    path: impl AsRef<Path>,
    tag: &Tag,
    version: Version,
) -> crate::Result<()> {
    let new_path = path.as_ref().with_extension("ID3TMP");
    let mut in_reader = BufReader::new(File::open(&path)?);
    let mut out_writer = BufWriter::new(File::create(&new_path)?);

    loop {
        // Read chunk ID
        let mut chunk_id: [u8; 4] = [0; 4];
        // EOF
        if in_reader.read(&mut chunk_id)? < 4 {
            break;
        }
        out_writer.write_all(&chunk_id)?;

        // Skip FORM chunk size & type
        if &chunk_id == b"FORM" {
            let mut buffer: [u8; 8] = [0; 8];
            in_reader.read_exact(&mut buffer)?;
            out_writer.write_all(&buffer)?;
            continue;
        }

        // Read chunk size
        let mut chunk_size_raw: [u8; 4] = [0; 4];
        if in_reader.read(&mut chunk_size_raw)? < 4 {
            break;
        }
        let chunk_size = u32::from_be_bytes(chunk_size_raw);

        // ID3 Chunk
        if &chunk_id[0..3] == b"ID3" {
            // Get ID3 bytes
            let mut id3_buffer = vec![];
            tag.write_to(&mut id3_buffer, version)?;

            let mut buffer = vec![];
            // Size
            buffer.extend(&(id3_buffer.len() as i32).to_be_bytes());
            // ID3 Data
            buffer.extend(id3_buffer);
            // Write
            out_writer.write_all(&buffer)?;

            // Seek main file
            in_reader.seek(SeekFrom::Current(chunk_size as i64))?;
            continue;
        }

        // Pass thru
        let mut buffer = vec![0; chunk_size as usize];
        in_reader.read_exact(&mut buffer)?;
        out_writer.write_all(&chunk_size_raw)?;
        out_writer.write_all(&buffer)?;
    }

    fs::remove_file(&path)?;
    fs::rename(&new_path, &path)?;

    Ok(())
}
