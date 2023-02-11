use std::io::{BufReader, BufRead, Seek, Read};
use std::ops::{Div, Add, Sub};
use std::str::FromStr;
use std::vec::Vec;
use std::path::{Path,PathBuf};
use std::fs::{File};
use log::{debug, info};
use std::cell::RefCell;
use std::time::Duration;

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
    print_duration: Option<PrintDurationEstimator>
}

const TIME: &str = ";TIME:";
const TIME_ELAPSED: &str = ";TIME_ELAPSED:";

struct PrintDurationEstimator {
    line_no_elapsed: std::vec::Vec<(u32, Duration)>,
    this_line_no_elapsed_idx: RefCell<usize>
}

impl PrintDurationEstimator{
    pub fn new() -> Self {
        return PrintDurationEstimator {line_no_elapsed:std::vec::Vec::new(), this_line_no_elapsed_idx: RefCell::new(0)};
    }
    
    pub fn count_points(&self) -> usize {
        return self.line_no_elapsed.len();
    }

    pub fn add_time_point(&mut self, time_point_secs: f64, at_lines: u32) {
        let bound = self.line_no_elapsed.iter()
        .enumerate()
        .find(|(_, item)| at_lines < item.0);

        match bound {
            Some((idx, _)) => {
                self.line_no_elapsed.insert(idx, (at_lines, Duration::from_secs_f64(time_point_secs)));
            },
            None => {
                self.line_no_elapsed.push((at_lines, Duration::from_secs_f64(time_point_secs)))
            }
        }
    }

    pub fn get_remaining_time(&self, cur_line_in_file: u32, cur_t: Duration) -> Duration {
        // Increment the index until the timepoint is after the current line
        while cur_line_in_file > self.line_no_elapsed[*self.this_line_no_elapsed_idx.borrow()].0 && *self.this_line_no_elapsed_idx.borrow() < self.line_no_elapsed.len() {
            *self.this_line_no_elapsed_idx.borrow_mut() += 1;
        }

        if *self.this_line_no_elapsed_idx.borrow() == self.line_no_elapsed.len() { // We're past the end - we should be done!
            return Duration::ZERO;
        }
        const ZERO: (u32, Duration) = (0u32, Duration::ZERO);

        // Find the slope of T1,L1 and T2,L2 and interpolate the current position
        let (l1, t1) = if *self.this_line_no_elapsed_idx.borrow() == 0 {&ZERO} else {&self.line_no_elapsed[*self.this_line_no_elapsed_idx.borrow() - 1]};
        let (l2, t2) = &self.line_no_elapsed[*self.this_line_no_elapsed_idx.borrow()];

        let slope = (t2.saturating_sub(*t1).as_secs_f64())
                                    .div((*l2 - *l1) as f64);
        let intercept = t2.as_secs_f64() - slope * (*l2 as f64);

        // Find the expected time point based on the interpolation
        let expected_t = Duration::from_secs_f64(slope * (cur_line_in_file as f64) + intercept);

        let mut remaining_t = self.line_no_elapsed.last().unwrap().1 - expected_t;

        if cur_t >= expected_t {
            // We're late - add the lateness to the remaining time
            remaining_t = remaining_t.add(cur_t.sub(expected_t));
        } else {
            // We're early! Subtract the earliness from the remaining time
            remaining_t = remaining_t.saturating_sub(expected_t.sub(cur_t));
        }

        return remaining_t;
    }

    pub fn get_last_time_point_duration(&self) -> (u32, Duration){
        self.line_no_elapsed.last().unwrap().clone()
    }
}

impl GCodeFile {
    fn parse_metadata<T: FromStr>(line: &str, prefix: &str) -> std::result::Result<T, T::Err> {
        line.trim_start_matches(prefix).parse::<T>()
    }

    pub fn new(gcode_file: &Path) -> std::io::Result<GCodeFile> {
        match File::open(gcode_file) {
            Err(e) => Err(e),
            Ok(f) => {
                let mut ret_file = GCodeFile{line_count:0 as u32, 
                    cur_line_in_file: 0, file: BufReader::new(f), 
                    path:gcode_file.to_path_buf(), 
                    last_line: String::new(), 
                    command_line_no: 0, 
                    resend_last:false,
                    print_duration: Some(PrintDurationEstimator::new())};

                let reader = BufReader::new(ret_file.file.by_ref());
                let mut total_time: f64 = -1.;
                
                for line in reader.lines() {
                    let line_str = line.unwrap();
                    ret_file.line_count += 1;

                    if line_str.starts_with(TIME) {
                        total_time = GCodeFile::parse_metadata::<f64>(&line_str, TIME).unwrap_or_default();
                    } else if line_str.starts_with(TIME_ELAPSED) {
                        let time_point = GCodeFile::parse_metadata::<f64>(&line_str, TIME_ELAPSED).unwrap_or_default();
                        ret_file.print_duration.as_mut().unwrap().add_time_point(time_point, ret_file.line_count);
                    }
                }

                if total_time > 0. {
                    ret_file.print_duration.as_mut().unwrap().add_time_point(total_time, ret_file.line_count);
                } else if ret_file.print_duration.as_ref().unwrap().count_points() == 0 {
                    ret_file.print_duration = None;
                }
                
                ret_file.file.rewind().unwrap();
                
                Ok(ret_file)
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
        self.path.file_name().unwrap_or_default().to_str().unwrap_or_default()
    }

    pub fn get_remaining_time(&self, cur_time_secs: Duration) -> Option<Duration> {
        match &self.print_duration {
            Some(estimator) => {
                Some(estimator.get_remaining_time(self.cur_line_in_file, cur_time_secs))
            }
            None => None
        }
    }

    pub fn get_duration_lines(&self) -> Option<(u32, Duration)> {
        match &self.print_duration {
            Some(estimator) => {
                Some(estimator.get_last_time_point_duration())
            }
            None => None
        }
    }
}


#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    const EPSILON: f64 = 0.01;

    #[test]
    fn estimator_one_time_point() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(10., 10);
        
        assert_approx_eq!(estimator.get_remaining_time(5, Duration::from_secs(5)).as_secs_f64(), 5., EPSILON);
    }

    #[test]
    fn estimator_two_time_points() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(180., 2000);
        estimator.add_time_point(312., 3676);

        let rem = estimator.get_remaining_time(3500, Duration::from_secs(296));
        assert_approx_eq!(rem.as_secs_f64(), 11.72, EPSILON);
    }

    #[test]
    fn estimator_two_time_points_slower() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(180., 2000);
        estimator.add_time_point(312., 3676);

        let rem = estimator.get_remaining_time(3400, Duration::from_secs(311));
        assert!(rem.as_secs_f64() > 1.);
    }

    #[test]
    fn estimator_two_time_points_faster() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(180., 2000);
        estimator.add_time_point(312., 3676);

        let rem = estimator.get_remaining_time(3500, Duration::from_secs(290));
        assert!(rem.as_secs_f64() < 312. - 290.);
    }

    #[test]
    fn estimator_two_time_points_way_faster() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(180., 2000);
        estimator.add_time_point(312., 3676);

        let rem = estimator.get_remaining_time(3500, Duration::from_secs(200));
        assert!(rem == Duration::ZERO);
    }

    #[test]
    fn estimator_time_point_later() {
        let mut estimator = PrintDurationEstimator::new();
        estimator.add_time_point(180., 2000);
        estimator.add_time_point(312., 3676);

        let rem = estimator.get_remaining_time(3500, Duration::from_secs(320));
        assert!(rem.as_secs_f64() < 312. - 250.);
    }

    #[test]
    fn estimator_real_file() {
        
        let mut file = GCodeFile::new(&std::env::current_dir().unwrap().join(Path::new("samples/PSME_cane-clip-2.gcode"))).expect("Cannot open sample file!");
        
        while file.get_progress().0 < 223400 {
            let _ = file.next_line().unwrap();
            
        }
        let remaining = file.get_remaining_time(Duration::from_secs(7792)).unwrap();

        assert_approx_eq!(remaining.as_secs_f64(), 60.45, EPSILON);
        println!("{:?}", remaining);
    }
}