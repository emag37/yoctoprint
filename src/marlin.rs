extern crate lazy_static;
use regex::Regex;

use crate::internal_api;
use crate::serial::*;
use internal_api::*;
use std::io::*;

lazy_static! {
    // Matches T:22.81 /0.00. Current in group 1, target in group 2
    static ref TEMP_DEG_REGEX: Regex = Regex::new(r"[TBCPLR][0-9]?:([0-9]+\.[0-9]+) +/([0-9]+\.[0-9]+)").unwrap();
    // Matches @:0
    static ref HEATER_POWER_REGEX: Regex = Regex::new(r"([BC]?@[0-9]?):([0-9]+)").unwrap();
    static ref RESIDENCY_REGEX: Regex = Regex::new(r"W:([\?0-9][0-9]*)").unwrap();
    static ref POSITION_REGEX: Regex = Regex::new(r"([XYZE]):(-?[0-9]+\.[0-9]+)").unwrap();
}



pub struct Marlin {
}

impl Marlin {
    fn parse_temperature(in_str: &str) -> std::io::Result<Vec<Temperature>> {
        let mut results : Vec<Temperature> = TEMP_DEG_REGEX.captures_iter(in_str)
        .filter(|cap| {cap.len() >= 2 && cap.get(0).is_some() && cap.get(0).unwrap().as_str().len() > 2})
        .map(|cap| {
            let point = match cap.get(0).unwrap().as_str().chars().nth(0) {
                Some('T') => {ProbePoint::HOTEND}
                Some('B') => {ProbePoint::BED}
                Some('C') => {ProbePoint::CHAMBER}
                Some('P') => {ProbePoint::PROBE}
                Some('L') => {ProbePoint::COOLER}
                Some('R') => {ProbePoint::REDUNDANT}
                _ => {ProbePoint::UNKNOWN}
            };

            let idx = match cap.get(0).unwrap().as_str().chars().nth(1) {
                Some(val@ '0'..='9') => {val.to_digit(10).unwrap()}
                _ => {0}
            };

            Temperature{measured_from:point, index: idx, power: 0, current: cap.get(1).unwrap().as_str().parse::<f64>().unwrap(), target: cap.get(2).unwrap().as_str().parse::<f64>().unwrap()}
        }).collect();

        if results.len() == 0 {
            return Err(std::io::Error::new(ErrorKind::NotFound, format!("No temperature values found in {in_str}")));
        }

        for cap in HEATER_POWER_REGEX.captures_iter(in_str) {
            let point = match cap.get(1).unwrap().as_str().chars().nth(0) {
                Some('B') => {ProbePoint::BED}
                Some('C') => {ProbePoint::CHAMBER}
                Some('@') => {ProbePoint::HOTEND}
                _ => {ProbePoint::UNKNOWN}
            };
            let id = match cap.get(1).unwrap().as_str().chars().nth(1) {
                Some(val@ '0'..='9') => {val.to_digit(10).unwrap()}
                _ => {0}
            };
            
            if let Some(mut res) = results.iter_mut().find(|elem| { elem.index == id && elem.measured_from == point}) {
                res.power = cap.get(2).unwrap().as_str().parse::<u8>().unwrap();
            }
        }
        Ok(results)
    }

    fn parse_position(in_str: &str) -> Result<Position>{
        let mut new_pos = Position{x:0.0, y: 0.0, z:0.0, e:0.0};
        
        for cap in POSITION_REGEX.captures_iter(in_str) {
            if cap.len() < 2 {
                println!("Cannot parse position: {:?}", cap);
                continue;
            }

            let val = cap.get(2).unwrap().as_str().parse::<f64>();

            if let Err(e) = val {
                println!("Error parsing value: {}", e);
                continue;
            }

            match cap.get(1).unwrap().as_str().chars().nth(0) {
                Some('X') => {new_pos.x = val.unwrap();},
                Some('Y') => {new_pos.y = val.unwrap();},
                Some('Z') => {new_pos.z = val.unwrap();},
                Some('E') => {new_pos.e = val.unwrap();},
                _ => {println!("Cannot parse position type {:?}", cap.get(1))}
            };
        }
        return Ok(new_pos);
    }
}


impl SerialProtocol for Marlin {
    fn parse_rx_line(&self, line: &str) -> std::io::Result<Response> {
        let trimmed_line = line.trim();
        if trimmed_line.trim().len() == 0 {
            return Ok(Response::NONE);
        } else if trimmed_line.starts_with("ok") {
            return Ok(Response::OK);
        } else if trimmed_line.contains("busy:") {
            return Ok(Response::BUSY);
        } else if trimmed_line.starts_with("T:") {
            let residency = 
            match
            match RESIDENCY_REGEX.captures(trimmed_line) {
                Some(cap) => {cap.get(1).unwrap().as_str()}
                None => {"?"}
            }.parse::<u32>() {
                Ok(val) => Some(val),
                Err(_) => None
            };


            return match Self::parse_temperature(line) {
                Ok(res) =>  {
                    Ok(Response::TEMPERATURE(res, residency)) 
                }
                Err(e) => {
                    println!("Could not parse {}", line);
                    Err(e)
                }
            }
            
        } else if trimmed_line.starts_with("X:") {
            return match Self::parse_position(line) {
                Ok(res) => {
                    Ok(Response::POSITION(res))
                }
                Err(e) => {
                    println!("Could not parse {}", line);
                    Err(e)
                }
            }
        }

        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Unknown rx line: {}", line)));
    }

    fn parse_outgoing_cmd(&self, out_cmd: &str) -> Option<OutgoingCmd> {
        if out_cmd.starts_with("G90") {
            Some(OutgoingCmd::PositionModeChange(PositionModeCmd::All(PositionMode::ABSOLUTE)))
        } else if out_cmd.starts_with("G91") {
            Some(OutgoingCmd::PositionModeChange(PositionModeCmd::All(PositionMode::RELATIVE)))
        } else if out_cmd.starts_with("M82") {
            Some(OutgoingCmd::PositionModeChange(PositionModeCmd::ExtruderOnly(PositionMode::ABSOLUTE)))
        } else if out_cmd.starts_with("M83") {
            Some(OutgoingCmd::PositionModeChange(PositionModeCmd::ExtruderOnly(PositionMode::RELATIVE)))
        } else {
            None
        }
    }

    fn get_home_cmds(&self) -> Vec<String> {
        vec!["M17".to_owned(), "G28".to_owned()]
    }

    fn get_set_temperature_cmds(&self, new_t: &TemperatureTarget) -> Vec<String> {
        let code = match new_t.to_set {
            ProbePoint::HOTEND => {"M104"}
            ProbePoint::BED => {"M140"}
            ProbePoint::CHAMBER => {"M141"}
            ProbePoint::COOLER => {"M143"}
            _ => {
                println!("Cannot set temperature for {:?}", new_t.to_set);
                return Vec::new();
            }
        };


        let index = format!("T{}",new_t.index.unwrap_or(0));
        let target = format!("S{}", new_t.target.round() as u32);
        vec![format!("{} {} {}", code, index, target)]
    }

    fn get_move_cmds(&self, new_pos: &Position, cur_pos_mode_az_e: (PositionMode, PositionMode)) -> Vec<String> {
        let mut cmds = vec!["G91".to_owned(),format!("G1 E{:.5} X{:.5} Y{:.5} Z{:.5}", new_pos.e, new_pos.x, new_pos.y, new_pos.z)];

        if matches!(cur_pos_mode_az_e.0, PositionMode::ABSOLUTE) {
            cmds.push("G90".to_owned());
        } else if matches!(cur_pos_mode_az_e.1, PositionMode::ABSOLUTE) {
            cmds.push("M82".to_owned());
        }
        return cmds;
    }

    fn get_enable_temperature_updates_cmds(&self, interval: std::time::Duration) -> Vec<String> {
        vec![format!("M155 S{}", interval.as_secs())]
    }
    
    fn add_message_frame(&self, line_no: u32, cmd: &str) -> String {
        format!("N{}{}")
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_temperature_line() {
        let test_line = " T:22.58 /0.00 B:23.11 /70.00 @:0 B@:0 W:?";
        let resp = Marlin{}.parse_rx_line(test_line);

        assert!(resp.is_ok());
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:0, current:22.58, target:0.0},
            Temperature{measured_from:ProbePoint::BED, index:0, power:0, current:23.11, target:70.0}], None));

        let test_line = " T:22.67 /66.66 B:23.11 /70.00 @:55 B@:127 W:30  ";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:55, current:22.67, target:66.66},
            Temperature{measured_from:ProbePoint::BED, index:0, power:127, current:23.11, target:70.0}], Some(30)));
    }

    #[test]
    fn parse_temperature_line_no_bed_no_residency() {
        let test_line = "T:22.67 /66.66 @:55";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:55, current:22.67, target:66.66}], None));
    }

    #[test]
    fn parse_position_line() {
        let test_line = "X:0.13 Y:152.00 Z:3.01 E:-3.95 Count X:0 Y:12160 Z:6060";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::POSITION(Position{x: 0.13, y: 152.00, z: 3.01, e: -3.95}));
    }

}