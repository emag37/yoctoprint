use crate::internal_api::Axis;
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
use enumset::{EnumSet,enum_set};

pub trait PrinterControl {
    fn read_from_printer(&mut self) -> std::io::Result<Response>;
    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize>;
    fn print_next_line(&mut self) -> std::io::Result<()>;
    fn poll_new_status(&mut self);
    fn get_status(&self) -> Result<internal_api::PrinterStatus>;
    fn get_state(&self) -> PrintState;
    fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn go_home(&mut self, axes: &EnumSet<Axis>) -> Result<()>;
    fn move_relative(&mut self, new_pos: &Position) -> Result<()>;
    fn set_temperature(&mut self, new_temp: &TemperatureTarget) -> Result<()>;
}

pub struct Printer {
    pub comms: serial::PrinterComms,
    pub protocol: Box<dyn SerialProtocol>,
    to_print: Option<file::GCodeFile>,
    homed_axes: EnumSet<Axis>,
    temperatures: Vec<Temperature>,
    position: Position,
    move_mode_xyz_e: (PositionMode, PositionMode),
    state: PrintState,
    is_busy: bool
}

impl PrinterControl for Printer {
    fn read_from_printer(&mut self) -> std::io::Result<Response>{
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

    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize> {
        self.comms.port.get_mut().write(format!("{}{}", data.trim_end(), '\n').as_bytes())
    }

    
    fn print_next_line(&mut self) -> std::io::Result<()> {
        if self.is_busy {
            self.poll_new_status();
            return Ok(())
        }
        if self.state != PrintState::STARTED {
            return Err(Error::new(std::io::ErrorKind::NotFound, format!("Printer is not in {:?} state ({:?})!", PrintState::STARTED, self.state)));
        }
        
        let (next_line_no, cmd) = 
        match self.to_print.as_mut().unwrap().next_line() {
            Ok(line) => {(line.0, line.1.to_owned())}
            Err(e) => {return Err(e);}
        };

        if cmd.len() == 0 {
            self.transition_state(PrintState::DONE);
            return Ok(());
        }

        if let Some(tapped_cmd) = self.protocol.parse_outgoing_cmd(&cmd) {
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

        self.send_cmd_read_until_response(&cmd, Some(next_line_no))
        
    }

    fn poll_new_status(&mut self) {
        loop {
            match self.read_from_printer() {
                Ok(resp) => {
                    match resp {
                        serial::Response::BUSY => {
                            self.is_busy = true;
                            break;
                        }
                        serial::Response::OK => {
                            self.is_busy = false;
                            break;
                        }
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

    fn get_status(&self) -> Result<internal_api::PrinterStatus>{
        Ok(internal_api::PrinterStatus{ 
            printer_connected: true,
            manual_control_enabled: self.can_move_manually(), 
            state: self.state, 
            temperatures: self.temperatures.clone(), 
            position: self.position, 
            gcode_lines_done_total: match &self.to_print {
                Some(p) => {Some((p.path.file_name().unwrap().to_str().unwrap().to_string(), p.cur_line_in_file, p.line_count))}
                None => None
            }})
    }

    fn get_state(&self) -> PrintState {
        return self.state;
    }
    
    fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()> {
        if self.state != PrintState::CONNECTED && self.state != PrintState::DONE {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Cannot set gcode file in this state ({:?})!", self.state)));
        }
       
        match file::GCodeFile::new(&abs_path) {

            Ok(f) => {
                self.to_print = Some(f);
                if let Err(e) = self.send_cmd_read_until_response(self.protocol.get_reset_line_no_cmd(0).as_str(), None){
                    return Err(e);
                }
                Ok(())
            }
            Err(e) => Err(e)
        }
    }

    fn start(&mut self) -> Result<()> {
        if self.state != PrintState::CONNECTED && self.state != PrintState::PAUSED && self.state != PrintState::DONE {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be started from this state ({:?})!", self.state)));
        }
        
        if self.state == PrintState::CONNECTED && self.to_print.is_none() {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("GCode file not loaded.")));
        }
        
        if self.to_print.is_some() && self.to_print.as_ref().unwrap().command_line_no != 0 && self.state != PrintState::PAUSED {
            let current_gcode_file_path = self.to_print.as_ref().unwrap().path.clone();
            if let Err(e) = self.set_gcode_file(&current_gcode_file_path) {
                return Err(e);
            }
        }

        //TODO: Restore position when resuming from pause
        self.transition_state(PrintState::STARTED);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED && self.state != PrintState::DONE && self.state != PrintState::PAUSED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be stopped from this state ({:?})!", self.state)));
        }

        let mut stop_sequence = || -> Result<()> {
            if self.is_busy {
                self.send_cmd_read_until_response(self.protocol.get_stop_cmd(false).as_str(), None)?;
            }
            
            self.send_cmd_read_until_response(self.protocol.get_fan_speed_cmd(0, 0.).as_str(), None)?;
            self.disable_all_heaters();
            self.transition_state(PrintState::CONNECTED);
            Ok(())
        };

        stop_sequence()
    }

    fn pause(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be paused from this state ({:?})!", self.state)));
        }

        //TODO: Save position
        self.transition_state(PrintState::PAUSED);
        Ok(())
    }

    fn go_home(&mut self, axes: &EnumSet<Axis>) -> Result<()> {
        if !matches!(self.state, PrintState::CONNECTED |  PrintState::DONE | PrintState::PAUSED) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be homed from this state ({:?})!", self.state)));
        }
        
        for cmd in self.protocol.get_home_cmds(&axes) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str(), None) {
                return Err(e);
            }
        }

        if self.state == PrintState::DONE {
            self.transition_state(PrintState::CONNECTED);
        }
        self.homed_axes |= *axes;
        
        Ok(())
    }

    fn move_relative(&mut self, new_pos: &Position) -> Result<()> {
        if !matches!(self.state, PrintState::CONNECTED |  PrintState::DONE | PrintState::PAUSED) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be moved from this state ({:?})!", self.state)));
        }
        if !self.can_move_manually() {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, "Printer cannot be moved manually, home it first?"));
        }

        if [new_pos.x, new_pos.y, new_pos.z].iter()
        .any(|coord| *coord > 20. || *coord < 0.) ||
        (new_pos.e > 100. || new_pos.e < 0.){
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Relative move beyond acceptable range!"));
        }

        println!("Set position to {:?}", new_pos);
        for cmd in self.protocol.get_move_cmds(new_pos, self.move_mode_xyz_e) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str(), None) {
                return Err(e);
            }
        }

        Ok(())
    }

    fn set_temperature(&mut self, new_temp: &TemperatureTarget) -> Result<()> {
        if matches!(self.state, PrintState::DEAD) {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Temperature cannot be modified from this state ({:?})!", self.state)));
        }

        if !self.temperatures.iter().map(|t|t.measured_from).any(|p| p == new_temp.to_set) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("Invalid heater {:?}",new_temp.to_set)));
        }

        if new_temp.target > 300. || new_temp.target < 0. {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid target temperature"));
        }

        println!("Set temperatures to: {:?}", new_temp);
        for cmd in self.protocol.get_set_temperature_cmds(new_temp) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str(), None) {
                return Err(e);
            }
        }

        Ok(())
    }
}

impl Printer {
    pub fn new(comms:PrinterComms) -> Result<Self> {
        if let Some(fw) = comms.fw_info.get("FIRMWARE_NAME") {
            if fw.to_lowercase().contains("marlin") {
                let mut ret_printer = Printer{comms, protocol:Box::new(marlin::Marlin{}), to_print: None, state: PrintState::CONNECTED,
                homed_axes:EnumSet::new(), temperatures: Vec::new(), position: Position::default(),
                move_mode_xyz_e: (PositionMode::ABSOLUTE, PositionMode::ABSOLUTE), is_busy: false};

                for cmd in ret_printer.protocol.get_enable_temperature_updates_cmds(std::time::Duration::from_secs(2)) {
                    if let Err(e) = ret_printer.send_cmd_read_until_response(cmd.as_str(), None) {
                        return Err(Error::new(std::io::ErrorKind::InvalidData, "Error probing initial temperatures."));
                    }
                }
                return Ok(ret_printer);
            } else {
                return Err(Error::new(std::io::ErrorKind::InvalidData, "Unsupported firmware type."));
            }
        }
            
        return Err(Error::new(std::io::ErrorKind::InvalidData, "Cannot find firmware type"));
    }

    fn transition_state(&mut self, new_state: PrintState) -> bool {
        if new_state == self.state {
            return true;
        }
        println!("Printer state transition: {:?} -> {:?}", self.state, new_state);
        self.homed_axes = EnumSet::new();
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

    fn send_cmd_read_until_response(&mut self, cmd: &str, line_no: Option<u32>) -> std::io::Result<()> {
        let to_send = 
        match line_no {
            Some(no) => {self.protocol.add_message_frame(no, cmd)}
            None => {cmd.to_string()}
        };

        if let Err(e) = self.send_to_printer(&to_send) {
            self.transition_state(PrintState::DEAD);
            return Err(e);
        }

        loop {
            match self.read_from_printer() {
                Ok(resp) => {
                    match resp {
                        serial::Response::NONE => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                            continue;
                        }
                        serial::Response::BUSY => {
                            self.is_busy = true;
                            break;
                        }
                        serial::Response::OK => {
                            self.is_busy = false;
                            break;
                        }
                        serial::Response::NACK(line) => {
                            self.is_busy = false;
                            if self.to_print.is_none() {
                                self.transition_state(PrintState::DEAD);
                                return Err(Error::new(std::io::ErrorKind::InvalidData, format!("The printer is requesting a resend of line {}, but we don't have a loaded GCODE file?", line)));
                            }
                            self.to_print.as_mut().unwrap().resend_gcode_line(line);
                        }
                        _ => {self.update_status_from_response(&resp);}
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::InvalidData {
                        println!("Ignoring unparseable line. {}", e);
                        continue;
                    }
                    self.transition_state(PrintState::DEAD);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn can_move_manually(&self) -> bool {
        self.homed_axes.is_superset(enum_set!(Axis::X | Axis::Y | Axis::Z))
    }
    
    fn disable_all_heaters(&mut self) {
        let cmds = self.temperatures.iter().map(|t| {
            self.protocol.get_set_temperature_cmds(&TemperatureTarget { to_set: t.measured_from, index: Some(t.index), target: 0. })
        }).flatten().collect::<Vec<String>>();

        for cmd in cmds {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str(), None) {
                println!("Got error: {:?}", e);
            }
        }
       
    }

}


pub struct SimulatedPrinter {
    to_print: Option<file::GCodeFile>,
    homed_axes: EnumSet<Axis>,
    temperatures: Vec<Temperature>,
    position: Position,
    state: PrintState,
    last_line_at : std::time::Instant,
    last_temp_update: std::time::Instant
}


impl SimulatedPrinter {
    pub fn new() -> Self {
        let init_temps = vec![Temperature{ measured_from: internal_api::ProbePoint::HOTEND, index: 0, power: 0, current: 25.0, target: 25.0 },
                                                Temperature{ measured_from: internal_api::ProbePoint::BED, index: 0, power: 0, current: 21.0, target: 21.0 }];
        SimulatedPrinter { to_print: None, state: PrintState::CONNECTED,
            homed_axes:EnumSet::new(), temperatures: init_temps, position: Position::default(), 
            last_line_at: std::time::Instant::now(),
            last_temp_update: std::time::Instant::now()}
    }
}

impl PrinterControl for SimulatedPrinter {
    fn read_from_printer(&mut self) -> std::io::Result<Response> {
        Ok(Response::NONE)
    }

    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize> {
        Ok(data.len())
    }

    fn print_next_line(&mut self) -> std::io::Result<()> {
        if self.to_print.is_none() {
            return Err(Error::new(std::io::ErrorKind::NotFound, "No printer"));
        } else if std::time::Instant::now() - self.last_line_at >= std::time::Duration::from_millis(200) {
            let mut to_print = self.to_print.as_mut().unwrap();

            if to_print.cur_line_in_file < to_print.line_count {
                to_print.cur_line_in_file += 1;
            } else if self.state == PrintState::STARTED {
                self.state = PrintState::DONE;
            }
            self.last_line_at = std::time::Instant::now();
        }
        Ok(())
    }

    fn poll_new_status(&mut self) {
        if std::time::Instant::now() - self.last_temp_update < std::time::Duration::from_millis(750) {
            return;
        }

        for temp in &mut self.temperatures {
            let adjust = 0.5 + rand::random::<f64>() * 0.75;
            if temp.current < temp.target {
                temp.current += adjust;
            } else {
                temp.current -= adjust;
            }
        }
        self.last_temp_update = std::time::Instant::now();
    }

    fn get_status(&self) -> Result<internal_api::PrinterStatus> {
        Ok(internal_api::PrinterStatus{ 
            printer_connected: true,
            manual_control_enabled: self.homed_axes.is_superset(enum_set!(Axis::X | Axis::Y | Axis::Z)), 
            state: self.state, 
            temperatures: self.temperatures.clone(), 
            position: self.position, 
            gcode_lines_done_total: match &self.to_print {
                Some(p) => {Some((p.path.file_name().unwrap().to_str().unwrap().to_string(), p.cur_line_in_file, p.line_count))}
                None => None
            }})
    }

    fn get_state(&self) -> PrintState {
        self.state
    }

    fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()> {
        match file::GCodeFile::new(abs_path) {
            Ok(file) => {
                self.to_print = Some(file);
                Ok(())
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    fn start(&mut self) -> Result<()> {
        self.state = PrintState::STARTED;
        self.homed_axes.clear();
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = PrintState::CONNECTED;

        for temp in &mut self.temperatures {
            temp.target = 0.;
        }
        self.homed_axes.clear();
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.state = PrintState::PAUSED;
        Ok(())
    }

    fn go_home(&mut self, axes: &EnumSet<Axis>) -> Result<()> {
        self.homed_axes.insert_all(*axes);
        Ok(())
    }

    fn move_relative(&mut self, new_pos: &Position) -> Result<()> {
        Ok(())
    }

    fn set_temperature(&mut self, new_temp: &TemperatureTarget) -> Result<()> {
       for temp in &mut self.temperatures {
        if temp.measured_from == new_temp.to_set {
            temp.target = new_temp.target;
            break;
        }
       }
       Ok(())
    }
}