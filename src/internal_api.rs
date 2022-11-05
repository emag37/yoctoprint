use crate::PathBuf;
use rocket::serde::{Serialize};
use serde::Deserialize;
 
#[derive(PartialEq)]
#[derive(Debug)]
#[derive(Copy, Clone)]
#[derive(Serialize, Deserialize)]
pub enum ProbePoint {
    HOTEND,
    BED,
    CHAMBER,
    PROBE,
    COOLER,
    REDUNDANT,
    UNKNOWN
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Default)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub e: f64
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub struct Temperature {
    pub measured_from: ProbePoint,
    pub index: u32, // For multiple extruders which report T0, T1, etc...
    pub power: u8,
    pub current: f64,
    pub target: f64
}

#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone, Deserialize)]
pub struct TemperatureTarget {
    pub to_set: ProbePoint,
    pub index: Option<u32>,
    pub target: f64
}


#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Copy, Clone)]
#[derive(Serialize)]
pub enum PrintState {
    CONNECTED,
    STARTED,
    PAUSED,
    DONE,
    DEAD
}

#[derive(Serialize, Debug)]
pub struct PrinterStatus {
    pub manual_control_enabled: bool,
    pub state: PrintState,
    pub temperatures: Vec<Temperature>,
    pub position: Position,
    pub gcode_lines_done_total: Option<(PathBuf, u32, u32)>,
}

impl Default for PrinterStatus {
    fn default() -> PrinterStatus {
        PrinterStatus { manual_control_enabled: false,state: PrintState::DEAD, temperatures: Vec::new(), gcode_lines_done_total: None, position: Position::default() }
    }
}


pub enum PrinterCommand {
    Connect(PathBuf, u32),
    Disconnect,
    SetGcodeFile(PathBuf),
    StartPrint,
    PausePrint,
    StopPrint,
    GetStatus,
    ManualMove(Position),
    Home,
    SetTemperature(TemperatureTarget)
}

pub enum PrinterResponse {
    GenericResult(std::io::Result<()>),
    Status(std::io::Result<PrinterStatus>),
}