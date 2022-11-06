use std::io::{BufReader, BufRead, Seek, Read};
use std::vec::Vec;
use std::path::{Path,PathBuf};
use std::fs::File;

pub const GCODE_DIR: &str = "gcode";

// Find all gcode files recursively starting from start_dir
pub fn find_gcode_files(start_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files : Vec<PathBuf> = Vec::new();

    if start_dir.is_dir() {
        for entry in std::fs::read_dir(start_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.append(&mut find_gcode_files(&path)?);
            } else if path.extension().is_some() && path.extension().unwrap().eq_ignore_ascii_case("gcode") {
                files.push(path);
            }
        }
    }
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
    pub cur_line: u32,
    pub file: BufReader<std::fs::File>,
    pub path: PathBuf,
    pub last_line: String
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
                
                Ok(GCodeFile{line_count:n_lines as u32, cur_line: 0, file: BufReader::new(f), path:gcode_file.to_path_buf(), last_line: String::new()})
            }
        }
    }

    pub fn next_line(&mut self) -> std::io::Result<String> {
        let mut ret_line = String::new();
        loop {
            match self.file.read_line(&mut ret_line) {
                Ok(n_read) => {
                    if n_read == 0 {
                        // EOF
                        return Ok(ret_line);
                    }
                    if let Some(semicolon_pos) = ret_line.find(";") {
                        ret_line.truncate(semicolon_pos);
                    }

                    self.cur_line +=1;
                    if ret_line.len() == 0 {
                        continue
                    }

                    self.last_line = ret_line.clone();
                    return Ok(ret_line);}
                Err(e) => {return Err(e)}
            };
        }
    }

    pub fn reset(&mut self) {
        self.file.rewind().unwrap();
        self.cur_line = 0;
    }

    pub fn get_progress(&self) -> (u32, u32, f64) {
        (self.cur_line, self.line_count, ((self.cur_line as f64) / (self.line_count as f64)) * 100.)
    }

    pub fn name(&self) -> &str {
        self.path.file_name().unwrap().to_str().unwrap()
    }
}