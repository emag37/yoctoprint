use crate::internal_api::Axis;
use crate::internal_api::ConsoleMessage;
use crate::internal_api::Position;
use crate::internal_api::Temperature;
use crate::internal_api::TemperatureTarget;
use crate::serial;
use crate::internal_api;
use crate::file;
use crate::marlin;

use std::ops::Div;
use std::time::Duration;
use std::vec;
use crossbeam::channel::{Sender, Receiver};
use serial::*;
use internal_api::PrintState;
use std::io::Error;
use std::io::{BufRead,Result};
use std::path::PathBuf;
use enumset::{EnumSet,enum_set};

pub trait PrinterControl {
    fn read_from_printer(&mut self) -> std::io::Result<Response>;
    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize>;
    fn next_action(&mut self) -> Result<()>;
    fn get_status(&self) -> Result<internal_api::PrinterStatus>;
    fn get_state(&self) -> PrintState;
    fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn go_home(&mut self, axes: &EnumSet<Axis>) -> Result<()>;
    fn move_relative(&mut self, new_pos: &Position) -> Result<()>;
    fn set_temperature(&mut self, new_temp: &TemperatureTarget) -> Result<()>;
    fn set_fan_speed(&mut self, index: u32, speed: f64) -> Result<()>;
    fn create_external_console(&mut self) -> (Sender<ConsoleMessage>, Receiver<ConsoleMessage>);
}

struct PrintTimer {
    last_update: std::time::Instant,
    duration: std::time::Duration
}

impl PrintTimer {
    pub fn new() -> Self {
        return PrintTimer { last_update: std::time::Instant::now(), duration: std::time::Duration::from_secs(0) }
    }

    // Add elapsed time to the print duration
    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        self.duration += now - self.last_update;
        self.last_update = now;
    }

    // Reset the reference to now, to skip any time we spent paused, for example.
    pub fn skip(&mut self) {
        self.last_update = std::time::Instant::now();
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.duration
    }
}

struct ExternalConsole {
    rx_out: Sender<ConsoleMessage>,
    tx_in: Receiver<ConsoleMessage>,

    pending_external_channels: Option<(Sender<ConsoleMessage>, Receiver<ConsoleMessage>)>,
    is_connected: bool,
}

impl ExternalConsole {
    const BUF_SIZE : usize = 256;

    pub fn new() -> ExternalConsole {
        let (rx_out, rx_in) = crossbeam::channel::bounded::<ConsoleMessage>(ExternalConsole::BUF_SIZE);
        let (tx_out, tx_in) = crossbeam::channel::bounded::<ConsoleMessage>(ExternalConsole::BUF_SIZE);

        return ExternalConsole{rx_out: rx_out, tx_in: tx_in, pending_external_channels: Some((tx_out, rx_in)),is_connected: false};
    }  

    pub fn is_connected(&self) -> bool {
        return self.is_connected;
    }

    pub fn get_ext_channels(&mut self) -> (Sender<ConsoleMessage>, Receiver<ConsoleMessage>) {
        if self.is_connected {
            warn!("Replacing connected external console channels.");
        }
        
        let ext_channels = match self.pending_external_channels.take() {
            Some(channels) => {
                let ret_channels = channels;
                self.pending_external_channels = None;
                ret_channels
            }
            None => {
                let (rx_out, rx_in) = crossbeam::channel::bounded::<ConsoleMessage>(ExternalConsole::BUF_SIZE);
                let (tx_out, tx_in) = crossbeam::channel::bounded::<ConsoleMessage>(ExternalConsole::BUF_SIZE);
                self.rx_out = rx_out;
                self.tx_in = tx_in;
                (tx_out, rx_in)
            }
        };
        self.is_connected = true;
        return ext_channels;
    }

    pub fn send_rx(&mut self, msg: String, is_echo: bool) {
        if !self.is_connected {
            return;
        }

        match self.rx_out.try_send(ConsoleMessage{line:msg, is_echo:is_echo}) {
            // Drop any message we don't have room for
            Err(e) => {
                if e.is_disconnected() {
                    self.is_connected = false;
                }
            }
            Ok(()) => {}
        }
    }

    pub fn get_tx(&mut self) -> Option<String> {
        if !self.is_connected {
            return None;
        }
        
        match self.tx_in.try_recv() {
            Err(e) => {
                if e.is_disconnected() {
                    self.is_connected = false;
                }
                None
            }
            Ok(msg) => Some(msg.line)
        }
    }
}

macro_rules! send_series_of_cmds_read_until_response {
    // The pattern for a single `eval`
    ($s:ident, $($e:expr),+) => {
        {
            trait SendSingleCmd {
                fn send_cmd(&self, printer: &mut Printer) -> std::io::Result<()>;
            }
            impl SendSingleCmd for String {
                fn send_cmd(&self, printer: &mut Printer) -> std::io::Result<()> {
                    printer.send_cmd_read_until_response(self.as_str(), None)
                }
            }
            trait SendMultipleCmds {
                fn send_cmd(&self, printer: &mut Printer) -> std::io::Result<()>;
            }
            impl SendMultipleCmds for Vec<String> {
                fn send_cmd(&self, printer: &mut Printer) -> std::io::Result<()> {
                    printer.send_cmds_read_until_response(&self, None)
                }
            }

            $(
                ($e).send_cmd($s)?;
            )*
        }
    };
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
    is_busy: bool,
    print_timer: PrintTimer,
    fan_speeds: Vec<f64>,
    external_console: ExternalConsole
}

impl PrinterControl for Printer {
    fn read_from_printer(&mut self) -> std::io::Result<Response>{
        let mut read_str: String = String::new();

        match self.comms.port.read_line(&mut read_str) {
            Ok(n_read) => {
                if n_read == 0 { 
                    return Ok(Response::NONE);
                }
                
                self.external_console.send_rx(read_str.clone(), false);
                return self.protocol.parse_rx_line(&read_str);
            } Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    return Ok(Response::NONE);
                }
                error!("Got error reading from serial port {} after", e);
                Err(e)
            }
        }
    }

    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize> {
        let to_write = format!("{}{}", data.trim_end(), '\n');

        self.external_console.send_rx(to_write.clone(), true);
        self.comms.port.get_mut().write(to_write.as_bytes())
    }

    fn get_status(&self) -> Result<internal_api::PrinterStatus>{
        let time_remaining = match &self.to_print {
            Some(p) => {
                p.get_remaining_time(self.print_timer.elapsed())
            }
            None => None
        };

        Ok(internal_api::PrinterStatus{ 
            printer_connected: true,
            manual_control_enabled: self.can_move_manually(), 
            state: self.state, 
            temperatures: self.temperatures.clone(), 
            position: self.position, 
            gcode_lines_done_total: match &self.to_print {
                Some(p) => {Some((p.name().to_string(), p.cur_line_in_file, p.line_count))}
                None => None
            },
            print_time_remaining: time_remaining,
            fan_speed: self.fan_speeds.clone()
        })
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
                self.print_timer = PrintTimer::new();
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
            self.print_timer.skip();
            if let Err(e) = self.set_gcode_file(&current_gcode_file_path) {
                return Err(e);
            }
        }

        if self.state == PrintState::PAUSED {
            send_series_of_cmds_read_until_response!(self,
                self.protocol.get_set_position_mode(&PositionMode::ABSOLUTE, &PositionMode::ABSOLUTE), 
                self.protocol.get_move_cmds(&self.position, false),
                self.protocol.get_recover_extruder_cmd(),
                self.protocol.get_set_position_mode(&self.move_mode_xyz_e.0, &self.move_mode_xyz_e.1));
        }
        
        self.transition_state(PrintState::STARTED);
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED && self.state != PrintState::DONE && self.state != PrintState::PAUSED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be stopped from this state ({:?})!", self.state)));
        }

        if self.is_busy {
            send_series_of_cmds_read_until_response!(self, self.protocol.get_stop_cmd(false));
        }

        send_series_of_cmds_read_until_response!(self,
            self.protocol.get_fan_speed_cmd(0, 0.)
        );

        let res = self.disable_all_heaters();

        self.transition_state(PrintState::CONNECTED);

        if !res.is_ok() {
            return res;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        if self.state != PrintState::STARTED {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, format!("Printer cannot be paused from this state ({:?})!", self.state)));
        }
        self.print_timer.update();
        
        send_series_of_cmds_read_until_response!(self,
            self.protocol.get_report_position_cmd(), 
            self.protocol.get_retract_extruder_cmd()
        );

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
        
        // The homed axes status of the printer will be updated when we parse the outgoing command
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

        info!("Set position to {:?}", new_pos);

        send_series_of_cmds_read_until_response!(self,
            self.protocol.get_set_position_mode(&PositionMode::RELATIVE, &PositionMode::RELATIVE),
            self.protocol.get_move_cmds(new_pos, true),
            self.protocol.get_set_position_mode(&self.move_mode_xyz_e.0, &self.move_mode_xyz_e.1)
        );

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

        info!("Set temperatures to: {:?}", new_temp);
        for cmd in self.protocol.get_set_temperature_cmds(new_temp) {
            if let Err(e) = self.send_cmd_read_until_response(cmd.as_str(), None) {
                return Err(e);
            }
        }

        Ok(())
    }

    fn set_fan_speed(&mut self, index: u32, speed: f64) -> Result<()> {
        info!("Set fan {} to speed {}", index, speed);

        if speed < 0. || speed > 1. {
            return Err(Error::new(std::io::ErrorKind::InvalidInput, "Fan speed must be between 0 and 1."));
        }

        if let Err(e) = self.send_cmd_read_until_response(self.protocol.get_fan_speed_cmd(index, speed).as_str(), None) {
            return Err(e);
        }
            
        Ok(())
    }

    fn next_action(&mut self) -> Result<()> {
        match self.external_console.get_tx() {
            Some(line) => {
                info!("Sending external command: {}", line);
                if let Err(e) = self.send_cmd_read_until_response(&line, None) {
                    error!("Error sending command {} - {}", line, e);
                }
            }
            None => {}
        }
        
        if self.state == PrintState::STARTED {
            return self.print_next_line();
        } else {
            self.poll_new_status();
            Ok(())
        }
    }

    fn create_external_console(&mut self) -> (Sender<ConsoleMessage>, Receiver<ConsoleMessage>) {
        self.external_console.get_ext_channels()
    }
}

impl Printer {
    pub fn new(comms:PrinterComms) -> Result<Self> {
        if let Some(fw) = comms.fw_info.get("FIRMWARE_NAME") {
            if fw.to_lowercase().contains("marlin") {
                let mut ret_printer = Printer{comms, protocol:Box::new(marlin::Marlin{}), to_print: None, state: PrintState::CONNECTED,
                homed_axes:EnumSet::new(), temperatures: Vec::new(), position: Position::default(),
                move_mode_xyz_e: (PositionMode::ABSOLUTE, PositionMode::ABSOLUTE), is_busy: false,
                print_timer: PrintTimer::new(),
                fan_speeds: vec![0.], external_console: ExternalConsole::new()};

                for cmd in ret_printer.protocol.get_enable_temperature_updates_cmds(std::time::Duration::from_secs(2)) {
                    if let Err(e) = ret_printer.send_cmd_read_until_response(cmd.as_str(), None) {
                        return Err(Error::new(std::io::ErrorKind::InvalidData, format!("Error probing initial temperatures: {e}")));
                    }
                }
                return Ok(ret_printer);
            } else {
                return Err(Error::new(std::io::ErrorKind::InvalidData, "Unsupported firmware type."));
            }
        }
            
        return Err(Error::new(std::io::ErrorKind::InvalidData, "Cannot find firmware type"));
    }

    fn print_next_line(&mut self) -> std::io::Result<()> {
        self.print_timer.update();
        
        if self.is_busy {
            self.poll_new_status();
            return Ok(())
        }
        
        if self.to_print.is_none() {
            self.transition_state(PrintState::DEAD);
            return Err(Error::new(std::io::ErrorKind::NotFound, "No file to print!"));
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

    fn transition_state(&mut self, new_state: PrintState) -> bool {
        if new_state == self.state {
            return true;
        }
        info!("Printer state transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;

        return true;
    }

    fn update_status_from_response(&mut self, resp: &serial::Response) {
        match resp {
            serial::Response::TEMPERATURE(temp, _residency)  => {
                self.temperatures = temp.clone();
            }
            serial::Response::POSITION(pos) => {
                self.position = pos.clone();
            } 
            _ => {}
        }
    } 

    fn send_cmd_read_until_response(&mut self, cmd: &str, line_no: Option<u32>) -> std::io::Result<()> {
        debug!("Send command: {}", cmd);
        
        if let Some(tapped_cmd) = self.protocol.parse_outgoing_cmd(&cmd) {
            match tapped_cmd {
                OutgoingCmd::PositionModeChange(mode_change) => {
                    match mode_change {
                        PositionModeCmd::All(m) => {
                            info!("Position mode for all axes changed to {:?}", m);
                            self.move_mode_xyz_e = (m,m);
                        },
                        PositionModeCmd::ExtruderOnly(m) => {
                            info!("Position mode for extruder changed to {:?}", m);
                            self.move_mode_xyz_e.1 = m;
                        },
                    }
                },
                OutgoingCmd::FanSpeedChange((idx, speed)) => {
                    if idx as usize >= self.fan_speeds.len() {
                        self.fan_speeds.resize((idx + 1) as usize, 0.);
                    }
                    self.fan_speeds[idx as usize] = speed;
                },
                OutgoingCmd::HomeAxes(axes) => {
                    self.homed_axes |= axes;
                }
            }
        }
 
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
                        warn!("Ignoring unparseable line. {}", e);
                        continue;
                    }
                    self.transition_state(PrintState::DEAD);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn send_cmds_read_until_response(&mut self, cmds: &Vec<String>, line_no: Option<u32>) -> std::io::Result<()> {
        let mut cur_line_no = line_no;
        for cmd in cmds {
            self.send_cmd_read_until_response(cmd, line_no)?;

            if cur_line_no.is_some() {
                cur_line_no = cur_line_no.map(|v| v+1);
            }
        }
        Ok(())
    }

    fn can_move_manually(&self) -> bool {
        self.homed_axes.is_superset(enum_set!(Axis::X | Axis::Y | Axis::Z)) || self.state == PrintState::PAUSED
    }
    
    fn disable_all_heaters(&mut self) -> std::io::Result<()> {
        let cmds = self.temperatures.iter().map(|t| {
            self.protocol.get_set_temperature_cmds(&TemperatureTarget { to_set: t.measured_from, index: Some(t.index), target: 0. })
        }).flatten().collect::<Vec<String>>();

        send_series_of_cmds_read_until_response!(self, cmds);
        Ok(())
    }

}


pub struct SimulatedPrinter {
    to_print: Option<file::GCodeFile>,
    homed_axes: EnumSet<Axis>,
    temperatures: Vec<Temperature>,
    position: Position,
    state: PrintState,
    last_line_at : std::time::Instant,
    last_temp_update: std::time::Instant,
    print_timer: PrintTimer,
    gcode_send_interval:std::time::Duration,
    fan_speeds: Vec<f64>,
    external_console : ExternalConsole
}


impl SimulatedPrinter {
    pub fn new() -> Self {
        let init_temps = vec![Temperature{ measured_from: internal_api::ProbePoint::HOTEND, index: 0, power: 0., current: 25.0, target: 25.0 },
                                                Temperature{ measured_from: internal_api::ProbePoint::BED, index: 0, power: 0., current: 21.0, target: 21.0 }];
        SimulatedPrinter { to_print: None, state: PrintState::CONNECTED,
            homed_axes:EnumSet::new(), temperatures: init_temps, position: Position::default(), 
            last_line_at: std::time::Instant::now(),
            last_temp_update: std::time::Instant::now(),
            print_timer: PrintTimer::new(),
            gcode_send_interval: Duration::ZERO,
            fan_speeds: vec![0.],
            external_console: ExternalConsole::new()
        }
    }
}

impl PrinterControl for SimulatedPrinter {
    fn read_from_printer(&mut self) -> std::io::Result<Response> {
        Ok(Response::NONE)
    }

    fn send_to_printer(&mut self, data: &str) -> std::io::Result<usize> {
        Ok(data.len())
    }

    fn next_action(&mut self) -> std::io::Result<()> {
        // Check for external console commands

        match self.external_console.get_tx() {
            Some(line) => {
                println!("Send to printer: {}", line);
                self.external_console.send_rx(line, true);
            },
            None => {}
        }
        // Send next line
        self.print_timer.update();
        if !self.to_print.is_none() && std::time::Instant::now() - self.last_line_at >= self.gcode_send_interval {
            let mut to_print = self.to_print.as_mut().unwrap();

            if to_print.cur_line_in_file < to_print.line_count {
                to_print.cur_line_in_file += 1;
            } else if self.state == PrintState::STARTED {
                self.state = PrintState::DONE;
            }
            self.last_line_at = std::time::Instant::now();
        }

        // Update status
        if std::time::Instant::now() - self.last_temp_update >= std::time::Duration::from_millis(750) {
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

        Ok(())
    }

    fn get_status(&self) -> Result<internal_api::PrinterStatus> {
        let time_remaining = match &self.to_print {
            Some(p) => {
                p.get_remaining_time(self.print_timer.elapsed())
            }
            None => None
        };

        Ok(internal_api::PrinterStatus{ 
            printer_connected: true,
            manual_control_enabled: self.homed_axes.is_superset(enum_set!(Axis::X | Axis::Y | Axis::Z)), 
            state: self.state, 
            temperatures: self.temperatures.clone(), 
            position: self.position, 
            gcode_lines_done_total: match &self.to_print {
                Some(p) => {Some((p.path.file_name().unwrap().to_str().unwrap().to_string(), p.cur_line_in_file, p.line_count))}
                None => None
            },
            print_time_remaining: time_remaining,
            fan_speed: self.fan_speeds.clone()})
    }

    fn get_state(&self) -> PrintState {
        self.state
    }

    fn set_gcode_file(&mut self, abs_path: &PathBuf) -> Result<()> {
        match file::GCodeFile::new(abs_path) {
            Ok(file) => {
                match &file.get_duration_lines() {
                    Some((lines, dur)) => {
                        self.gcode_send_interval = dur.div(*lines);
                    },
                    None => {
                        self.gcode_send_interval = Duration::from_millis(20);
                    }
                }
                self.to_print = Some(file);
                Ok(())
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    fn start(&mut self) -> Result<()> {
        if self.state == PrintState::PAUSED {
            self.print_timer.skip();
        } else {
            self.print_timer = PrintTimer::new()
        }
        self.state = PrintState::STARTED;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = PrintState::CONNECTED;

        for temp in &mut self.temperatures {
            temp.target = 0.;
        }
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.print_timer.update();
        self.state = PrintState::PAUSED;
        Ok(())
    }

    fn go_home(&mut self, axes: &EnumSet<Axis>) -> Result<()> {
        self.homed_axes.insert_all(*axes);
        Ok(())
    }

    fn move_relative(&mut self, _new_pos: &Position) -> Result<()> {
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

    fn set_fan_speed(&mut self, index: u32, speed: f64) -> Result<()> {
        let vec_idx = index as usize;
        if vec_idx > self.fan_speeds.len() {
            self.fan_speeds.resize((vec_idx + 1) as usize, 0.);
        }

        self.fan_speeds[vec_idx] = speed;
        Ok(())
    }

    fn create_external_console(&mut self) -> (Sender<ConsoleMessage>, Receiver<ConsoleMessage>) {
        self.external_console.get_ext_channels()
    }
}