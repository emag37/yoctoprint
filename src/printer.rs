use crate::internal_api::Position;
use crate::internal_api::Temperature;
use crate::internal_api::TemperatureTarget;
use crate::serial;
use crate::internal_api;
use crate::file;
use crate::marlin;
use serial::*;
use internal_api::PrintState;
use std::io::Error;
use std::io::{BufRead,Result};
use std::path::PathBuf;

pub struct Printer {
    pub comms: serial::PrinterComms,
    pub protocol: Box<dyn SerialProtocol>,
    to_print: Option<file::GCodeFile>,
    can_move_manually: bool,
    temperatures: Vec<Temperature>,
    position: Position,
    move_mode_xyz_e: (PositionMode, PositionMode),
    state: PrintState
}

impl Printer {
    pub fn new(comms:PrinterComms) -> Option<Self> {
        if let Some(fw) = comms.fw_info.get("FIRMWARE_NAME") {
            if fw.to_lowercase().contains("marlin") {
                let mut ret_printer = Printer{comms, protocol:Box::new(marlin::Marlin{}), to_print: None, state: PrintState::CONNECTED,
                can_move_manually:false, temperatures: Vec::new(), position: Position::default(),
                move_mode_xyz_e: (PositionMode::ABSOLUTE, PositionMode::ABSOLUTE)};

                for cmd in ret_printer.protocol.get_enable_temperature_updates_cmds(std::time::Duration::from_secs(2)) {
                    ret_printer.send_cmd_read_until_response(cmd.as_str());
                }
                return Some(ret_printer);
            } else {
                println!("Unsupported firmware type.");
            }
        } else {
            println!("Cannot find firmware type.");
        }
        None
    }

    fn transition_state(&mut self, new_state: PrintState) -> bool {
        if new_state == self.state {
            return true;
        }
        println!("Printer state transition: {:?} -> {:?}", self.state, new_state);
        self.can_move_manually = false;
        return true;
    }

    fn update_status_from_response(&mut self, resp: &serial::Response) {
        match resp {
            serial::Response::TEMPERATURE(temp, residency)  => {
                self.temperatures = temp.clone();
            }
            serial::Response::POSITION(pos) => {
                self.position = pos.clone();
            } 
            _ => {}
        }
    }

    pub fn read_from_printer(&mut self) -> std::io::Result<Response>{
        let mut read_str: String = String::new();

        match self.comms.port.read_line(&mut read_str) {
            Ok(n_read) => {
                if n_read == 0 { 
                    return Ok(Response::NONE);
                }

                return self.protocol.parse_rx_line(&read_str);
            } Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    return Ok(Response::NONE);
                }
                println!("Got error reading from serial port {} after", e);
                Err(e)
            }
        }
    }

    pub fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize> {
        self.comms.port.get_mut().write(format!("{}{}", data.trim_end(), '\n').as_bytes())
    }

    fn send_cmd_read_until_response(&mut self, cmd: &str) -> std::io::Result<()> {
        if let Err(e) = self.send_to_printer(cmd) {
            self.transition_state(PrintState::DEAD);
            return Err(e);
        }

        loop {
            match self.read_from_printer() {
                Ok(resp) => {
                    match resp {
                        serial::Response::NONE | serial::Response::BUSY => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            continue;
                        }
                        serial::Response::OK => {
                            break;
                        }
                        _ => {self.update_status_from_response(&resp);}
                    }
                }
                Err(e) => {
                    self.transition_state(PrintState::DEAD);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn print_next_line(&mut self) -> std::io::Result<()> {
        if self.state != PrintState::STARTED {
            return Err(Error::new(std::io::ErrorKind::NotFound, format!("Printer is not in {:?} state ({:?})!", PrintState::STARTED, self.state)));
        }
        
        let next_line = 
        match self.to_print.as_mut().unwrap().next_line() {
            Ok(line) => {line}
            Err(e) => {return Err(e);}
        };

        if next_line.len() == 0 {
            self.transition_state(PrintState::DONE);
            return Ok(());
        }

        if let Some(tapped_cmd) = self.protocol.parse_outgoing_cmd(next_line.as_str()) {
           match tapped_cmd {
                OutgoingCmd::PositionModeChange(mode_change) => {
                    match mode_change {
                        PositionModeCmd::All(m) => {
                            println!("Position mode for all axes changed to {:?}", m);
                            self.move_mode_xyz_e = (m,m);
                        },
                        PositionModeCmd::ExtruderOnly(m) => {
                            println!("Position mode for extruder changed to {:?}", m);
                            self.move_mode_xyz_e.1 = m;
                        },
                    }
                },
            }
        }

        self.send_cmd_read_until_response(next_line.as_str())
        
    }

    pub fn poll_new_status(&mut self) {
        loop {
            match self.read_from_printer() {
                Ok(resp) => {
                    match resp {
                        serial::Response::NONE => {break;}
                        _ => {self.update_status_from_response(&resp);}
                    }
                }
                Err(e) => {
                    break;
                }
            }
        }
    }

    pub fn get_status(&self) -> Result<internal_api::PrinterStatus>{
        Ok(internal_api::PrinterStatus{ manual_control_enabled: self.can_move_manually, 
            state: self.state, 
            temperatures: self.temperatures.clone(), 
            position: self.position, 
            gcode_lines_done_total: match &self.to_print {
                Some(p) => {Some((p.path.clone(), p.cur_line, p.line_count))}
                None => None
            }})
    }

    pub fn get_state(&self) -> PrintState{
        return self.state;
    }
    
    pub fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()> {
        if self.state != PrintState::CONNECTED && self.state != PrintState::DONE {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Cannot set gcode file in this state ({:?})!", self.state)));
        }
       
        match file::GCodeFile::new(&abs_path) {

            Ok(f) => {
                self.to_print = Some(f);
                Ok(())
            }
            Err(e) => Err(e)
        }
    }

    pub fn start(&mut self) -> Result<()> {
        if self.state != PrintState::CONNECTED && self.state != PrintState::PAUSED && self.state != PrintState::DONE {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be started from this state ({:?})!", self.state)));
        }
        
        if self.state == PrintState::CONNECTED && self.to_print.is_none() {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("GCode file not loaded.")));
        }
        
        if self.state == PrintState::DONE && self.to_print.is_some() {
            self.to_print.as_mut().unwrap().reset();
        }

        //TODO: Restore position when resuming from pause
        self.transition_state(PrintState::STARTED);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED && self.state != PrintState::DONE && self.state != PrintState::PAUSED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be stopped from this state ({:?})!", self.state)));
        }

        self.transition_state(PrintState::CONNECTED);
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be paused from this state ({:?})!", self.state)));
        }

        //TODO: Save position
        self.transition_state(PrintState::PAUSED);
        Ok(())
    }

    pub fn go_home(&mut self) -> Result<()> {
        if !matches!(self.state, PrintState::CONNECTED |  PrintState::DONE | PrintState::PAUSED) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be homed from this state ({:?})!", self.state)));
        }
        
        for cmd in self.protocol.get_home_cmds() {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str()) {
                return Err(e);
            }
        }

        if self.state == PrintState::DONE {
            self.transition_state(PrintState::CONNECTED);
        }
        self.can_move_manually = true;
        
        Ok(())
    }

    pub fn move_relative(&mut self, new_pos: &Position) -> Result<()> {
        if !matches!(self.state, PrintState::CONNECTED |  PrintState::DONE | PrintState::PAUSED) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be moved from this state ({:?})!", self.state)));
        }
        if !self.can_move_manually {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, "Printer cannot be moved manually, home it first?"));
        }

        println!("Set position to {:?}", new_pos);
        for cmd in self.protocol.get_move_cmds(new_pos, self.move_mode_xyz_e) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str()) {
                return Err(e);
            }
        }

        Ok(())
    }

    pub fn set_temperature(&mut self, new_temp: &TemperatureTarget) -> Result<()> {
        if matches!(self.state, PrintState::DEAD) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Temperature cannot be modified from this state ({:?})!", self.state)));
        }

        println!("Set temperatures to: {:?}", new_temp);
        for cmd in self.protocol.get_set_temperature_cmds(new_temp) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str()) {
                return Err(e);
            }
        }

        Ok(())
    }
}