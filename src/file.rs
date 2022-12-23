use std::io::{BufReader, BufRead, Seek, Read};
use std::vec::Vec;
use std::path::{Path,PathBuf};
use std::fs::{File};
use log::{debug, info, error, warn};

use crate::internal_api::{self, FileInfo};

pub const GCODE_DIR: &str = "gcode";

// Find all gcode files recursively starting from start_dir
pub fn find_gcode_files(start_dir: &Path) -> std::io::Result<Vec<internal_api::FileInfo>> {
    let mut files : Vec<internal_api::FileInfo> = Vec::new();

    debug!("Looking for gcode files starting from {:?}", start_dir);
    if start_dir.is_dir() {
        for entry in std::fs::read_dir(start_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                debug!("Found directory {:?}", path);
                files.append(&mut find_gcode_files(&path)?);
            } else if path.extension().is_some() && path.extension().unwrap().eq_ignore_ascii_case("gcode") {
                let metadata = std::fs::metadata(&path).unwrap();
                debug!("Found a gcode file {:?}", path);
                files.push(FileInfo{path: path, size: metadata.len(), last_modified_since_epoch: metadata.modified()
                    .unwrap_or(std::time::UNIX_EPOCH)
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()});
            }
        }
    }

    info!("Found {} GCode files", files.len());
    
    Ok(files)
}

// Convert a gcode file into an absolute path if it isn't already.
pub fn get_abs_gcode_path(base_path: &PathBuf, file: &PathBuf) -> PathBuf {
    // Accept both with and without base path
    if !file.is_absolute() {
        return base_path.join(GCODE_DIR).join(file);
    }
    return file.clone();
}

pub struct GCodeFile {
    pub line_count: u32,
    pub cur_line_in_file: u32,
    pub file: BufReader<std::fs::File>,
    pub path: PathBuf,
    pub last_line: String,
    pub command_line_no: u32, // Keeps track of lines of actual GCode commands
    pub resend_last: bool,
}

impl GCodeFile {
    pub fn new(gcode_file: &Path) -> std::io::Result<GCodeFile> {
        match File::open(gcode_file) {
            Err(e) => Err(e),
            Ok(mut f) => {
                let mut n_lines : u32 = 0;
                {
                    let reader = BufReader::new(f.by_ref());
                    n_lines = reader.lines().count() as u32;
                }
                f.rewind().unwrap();
                
                Ok(GCodeFile{line_count:n_lines as u32, cur_line_in_file: 0, file: BufReader::new(f), path:gcode_file.to_path_buf(), last_line: String::new(), command_line_no: 0, resend_last:false})
            }
        }
    }
    
    pub fn resend_gcode_line(&mut self, gcode_lineno: u32) {
        // If we NACK the last line, just mark it to be replayed, since we buffered it
        if self.cur_line_in_file == gcode_lineno {
            self.resend_last = true;
        } else {
            self.file.rewind().expect("Failed to rewind file!");
            self.cur_line_in_file = 0;
            self.command_line_no = 0;

            while self.command_line_no < gcode_lineno - 1 {
                self.next_line().expect("Failed to fetch next line");
            }
        }
    }

    pub fn next_line(&mut self) -> std::io::Result<(u32, &str)> {
        if self.resend_last {
            self.resend_last = false;
            return Ok((self.command_line_no, &self.last_line));
        }

        let mut ret_line = String::new();
        loop {
            match self.file.read_line(&mut ret_line) {
                Ok(n_read) => {
                    if n_read == 0 {
                        // EOF
                        return Ok((self.command_line_no, ""));
                    }
                    if let Some(semicolon_pos) = ret_line.find(";") {
                        ret_line.truncate(semicolon_pos);
                    }

                    self.cur_line_in_file +=1;
                    if ret_line.len() == 0 {
                        continue
                    }

                    self.last_line = ret_line.trim_end().to_string();
                    self.command_line_no += 1;

                    return Ok((self.command_line_no, &self.last_line));
                }
                Err(e) => {return Err(e)}
            };
        }
    }

    pub fn get_progress(&self) -> (u32, u32, f64) {
        (self.cur_line_in_file, self.line_count, ((self.cur_line_in_file as f64) / (self.line_count as f64)) * 100.)
    }

    pub fn name(&self) -> &str {
        self.path.file_name().unwrap().to_str().unwrap()
    }
}