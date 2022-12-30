extern crate lazy_static;
use regex::Regex;
use serialport::SerialPort;
use std::io::*;
use crate::internal_api;
use enumset::{EnumSet, EnumSetType};
use internal_api::*;
use log::{debug, info, error, warn};

#[derive(Debug)]
#[derive(PartialEq)]
pub enum Response {
    NONE,
    BUSY,
    OK,
    TEMPERATURE(Vec<Temperature>, Option<u32>),
    POSITION(Position),
    NACK(u32)
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum PositionMode {
    ABSOLUTE,
    RELATIVE
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum PositionModeCmd {
    All(PositionMode),
    ExtruderOnly(PositionMode)
}

#[derive(Debug)]
#[derive(Copy, Clone)]
pub enum OutgoingCmd {
    PositionModeChange(PositionModeCmd),
    FanSpeedChange((u32, f64))
}

pub trait SerialProtocol {
    fn parse_rx_line(&self, in_str: &str) -> std::io::Result<Response>;
    fn parse_outgoing_cmd(&self, out_cmd: &str) -> Option<OutgoingCmd>;
    fn get_home_cmds(&self, axes : &EnumSet<internal_api::Axis>) -> Vec<String>;
    fn get_set_temperature_cmds(&self, new_t: &TemperatureTarget) -> Vec<String>;
    fn get_move_cmds(&self, new_pos: &Position, cur_pos_mode_az_e: (PositionMode, PositionMode)) -> Vec<String>;
    fn get_enable_temperature_updates_cmds(&self, interval: std::time::Duration) -> Vec<String>;
    // Adds metadata to a command, e.g: Line number and checksum for Marlin
    fn add_message_frame(&self, line_no: u32, cmd: &str) -> String;
    fn get_reset_line_no_cmd(&self, line_no: u32) -> String;
    fn get_stop_cmd(&self, emergency: bool) -> String;
    fn get_fan_speed_cmd(&self, index:u32, speed: f64) -> String;
    fn get_save_position_cmd(&self) -> String;
    fn get_restore_position_cmd(&self) -> String;
}

pub struct PrinterComms {
    pub port: std::io::BufReader<Box<dyn SerialPort>>,
    pub fw_info: std::collections::HashMap<String, String>,
}

impl PrinterComms {
    pub fn new(path: &str, baud: u32) -> std::io::Result<PrinterComms> {
        debug!("Trying port {} with baud rate {}", path, baud);

        if let Ok(test_port) = serialport::new(path, baud).open() {
            let mut new_port = PrinterComms{port: BufReader::new(test_port), fw_info: std::collections::HashMap::new()};
            if let Ok(reply) = new_port.send_cmd_await_result("M115", &std::time::Duration::from_millis(10)) {
                if reply.contains("FIRMWARE_NAME") { 
                    new_port.parse_fw_info(&reply);
                    info!("Got response {} on port {} with baud rate {}",  reply, path, baud);
                    return Ok(new_port);
                }
            }
        }

        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No printer found @ port {} with baud {}", path, baud)
        ));
    }

    fn purge_read(&mut self) {
        let mut readbuf: [u8; 1024] = [0; 1024];
        
        loop {
            match self.port.read(&mut readbuf) {
                Ok(n_read) => {
                    if n_read == 0 { break; }
                } 
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::TimedOut {
                     error!("Got error reading from serial port {}", e);
                    }
                    break;
                }
            };
        }
    }
    
    fn parse_fw_info(&mut self, m115_reply: &str) {
        let mut lines = m115_reply.lines();

        
        if let Some(first_line) = lines.next() {
            let kw_posns : Vec<(usize, &str)> = Regex::new("[A-Z]+(_[A-Z]+)*:").unwrap().find_iter(first_line)
            .map(|kw_match| (kw_match.start(), kw_match.as_str()))
            .collect();
            
            for pair_vals in kw_posns.windows(2) {
                let (pos, kw) = pair_vals[0];
                self.fw_info.insert(kw.trim_end_matches(':').to_string(), first_line[pos + kw.len()..pair_vals[1].0 - 1].to_string());
            }
            let (last_elem_pos, last_elem_kw) = kw_posns[kw_posns.len() - 1];
            self.fw_info.insert(last_elem_kw.to_string(), first_line[last_elem_pos + last_elem_kw.len() + 1..].to_string());
        }

        for cap_line in lines {
            if !cap_line.starts_with("Cap") {
                println!("Expected line to start with Cap, but got {}", cap_line);
                continue;
            }

            let kv : Vec<&str> = cap_line.split(':').filter(|token| *token != "Cap").collect();
            if kv.len() != 2 {
                println!("Expected key:value in line {}!", cap_line);
                continue;
            }
            self.fw_info.insert(kv[0].to_string(), kv[1].to_string());
        }
        info!("Got firmware info: {:?}", self.fw_info);
    }

    pub fn send_cmd_await_result(&mut self, cmd: &str, timeout: &std::time::Duration) -> std::io::Result<String> {
        let mut readbuf: [u8; 1024] = [0; 1024];
        let mut start_at =  std::time::Instant::now();
        let mut ret_str = String::new();
        
        self.purge_read();

        debug!("Write cmd: {}", cmd);
        if let Err(e) = self.port.get_mut().write(format!("{}{}", cmd, '\n').as_bytes()) {
            return Err(e);
        }
        let sent_at = std::time::Instant::now();
        
        while std::time::Instant::now() - start_at < *timeout {
            let result = match self.port.read(&mut readbuf) {
                Ok(n_read) => {
                    if n_read == 0 { continue; }

                    match std::str::from_utf8(&readbuf[0..n_read]) {
                        Ok(ret_str) => {Ok(ret_str)}
                        Err(e) => { 
                            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, 
                                format!("Error {} decoding data from serial port {:?}, will assume command failed", e, &readbuf[0..n_read])))
                        }
                    }
                } Err(e) => {
                    if e.kind() == std::io::ErrorKind::TimedOut {
                        continue;
                    }
                    println!("Got error reading from serial port {} after", e);
                    Err(e)
                }
            };

            match result {
                Ok(new_str) => {
                    println!("Got response {} after {:.3} secs", new_str, sent_at.elapsed().as_secs_f64());
                    start_at = std::time::Instant::now();
                    ret_str.push_str(&new_str);
                    if ret_str.contains("ok") {
                        return Ok(ret_str);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            }
            
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timed out waiting for printer to reply!"))
    }

    pub fn path(&self) -> String {
        return self.port.get_ref().name().unwrap_or(String::new());
    }

}

pub fn find_printer() -> std::io::Result<PrinterComms> {
    const BAUD_RATES: &'static [u32] = &[256000, 115200, 57600, 38400, 19200, 14400, 12800, 9600];

    match serialport::available_ports() {
        Ok(ports) => {
            for port in ports {
                debug!("Found serial port: {}", port.port_name);
                for baud in BAUD_RATES {
                    match PrinterComms::new(port.port_name.as_str(), *baud) {
                        Ok(comms) => {return Ok(comms)}
                        Err(_) => {}
                    }
                }
            }
        }
        Err(_) => {error!("Cannot scan ports!")}
    }

    return Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Failed to scan ports"
    ));

}