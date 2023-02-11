extern crate lazy_static;
use enumset::enum_set;
use regex::Regex;

use crate::internal_api;
use crate::serial::*;
use internal_api::*;
use std::io::*;
use enumset::EnumSet;

lazy_static! {
    // Matches T:22.81 /0.00. Current in group 1, target in group 2
    static ref TEMP_DEG_REGEX: Regex = Regex::new(r"[TBCPLR][0-9]?:([0-9]+\.[0-9]+) +/([0-9]+\.[0-9]+)").unwrap();
    // Matches @:0
    static ref HEATER_POWER_REGEX: Regex = Regex::new(r"([BC]?@[0-9]?):([0-9]+)").unwrap();
    static ref RESIDENCY_REGEX: Regex = Regex::new(r"W:([\?0-9][0-9]*)").unwrap();
    static ref POSITION_REGEX: Regex = Regex::new(r"([XYZE]):(-?[0-9]+\.[0-9]+)").unwrap();
    static ref LAST_LINE_REGEX: Regex = Regex::new(r"Last Line: ?([0-9]+)").unwrap();
}



pub struct Marlin {
}

impl Marlin {
    fn parse_temperature(in_str: &str) -> std::io::Result<Vec<Temperature>> {
        let mut results : Vec<Temperature> = TEMP_DEG_REGEX.captures_iter(in_str)
        .filter(|cap| {cap.len() >= 3 && cap.get(0).is_some() && cap.get(0).unwrap().as_str().len() > 2})
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

            Temperature{measured_from:point, index: idx, power: 0., current: cap.get(1).unwrap().as_str().parse::<f64>().unwrap(), target: cap.get(2).unwrap().as_str().parse::<f64>().unwrap()}
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
                res.power = cap.get(2).unwrap().as_str().parse::<f64>().unwrap() / 127.;
            }
        }
        Ok(results)
    }

    fn parse_position(in_str: &str) -> Result<Position>{
        let mut new_pos = Position{x:0.0, y: 0.0, z:0.0, e:0.0};
        
        for cap in POSITION_REGEX.captures_iter(in_str) {
            if cap.len() < 3 {
                error!("Cannot parse position: {:?}", cap);
                continue;
            }
            println!("{}", cap.len());
            let val = cap.get(2).unwrap().as_str().parse::<f64>();

            if let Err(e) = val {
                error!("Error parsing value: {}", e);
                continue;
            }

            match cap.get(1).unwrap().as_str().chars().nth(0) {
                Some('X') => {new_pos.x = val.unwrap();},
                Some('Y') => {new_pos.y = val.unwrap();},
                Some('Z') => {new_pos.z = val.unwrap();},
                Some('E') => {new_pos.e = val.unwrap();},
                _ => {error!("Cannot parse position type {:?}", cap.get(1))}
            };
        }
        return Ok(new_pos);
    }

    fn parse_home_cmd(&self, in_str: &str) -> EnumSet<Axis> {
        const ALL_VALID: EnumSet::<Axis> = enum_set!(Axis::X | Axis::Y | Axis::Z);

        if in_str.trim_end() == "G28" {
            return ALL_VALID;
        }

        let mut ret_set = EnumSet::<Axis>::empty();

        for segment in in_str.split(' ') {
            match segment.to_uppercase().as_str() {
                "X" => ret_set |= Axis::X,
                "Y" => ret_set |= Axis::Y,
                "Z" => ret_set |= Axis::Z,
                "0" => ret_set |= ALL_VALID,
                _ => {}
            }
        }

        ret_set
    }

    fn parse_fan_speed(&self, in_str: &str) -> (u32, f64) {
        let mut ret_idx = 0u32;
        let mut ret_speed = 0.;

        for segment in in_str.split(' ') {
            match segment.chars().nth(0) {
                Some('P') => {
                    ret_idx = match segment[1..].parse::<u32>() {
                        Ok(val) => val,
                        Err(_) => {
                            error!("Cannot parse Fan index from {}", segment);
                                0u32
                            }
                        }
                },
                Some('S') => {
                    ret_speed = match segment[1..].parse::<f64>() {
                        Ok(val) => val,
                        Err(_) => {
                            error!("Cannot parse Fan speed from {}", segment);
                            0.
                        }
                    };
                    ret_speed /= 255.;
                },
                Some(_) | None => {}
            }
        }
        (ret_idx, ret_speed)
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
                    error!("Could not parse {}", line);
                    Err(e)
                }
            }
            
        } else if trimmed_line.starts_with("X:") {
            return match Self::parse_position(line) {
                Ok(res) => {
                    Ok(Response::POSITION(res))
                }
                Err(e) => {
                    error!("Could not parse {}", line);
                    Err(e)
                }
            }
        } else if let Some(capture) = LAST_LINE_REGEX.captures(trimmed_line) {
            return Ok(Response::NACK(capture.get(1).unwrap().as_str().parse::<u32>().unwrap() + 1));
        } else if trimmed_line.starts_with("Resend: ") { // Ignore Resend, we'll use the line number in the previous line
            return Ok(Response::NONE);
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
        } else if out_cmd.starts_with("M106") || out_cmd.starts_with("M107") {
            Some(OutgoingCmd::FanSpeedChange(self.parse_fan_speed(out_cmd)))
        } else if out_cmd.starts_with("G28") {
            Some(OutgoingCmd::HomeAxes(self.parse_home_cmd(out_cmd)))
        } else {
            None
        }
    }
    
    fn get_home_cmds(&self, axes : &EnumSet<internal_api::Axis>) -> Vec<String> {
        let mut home_cmd = "G28 ".to_owned();
        
        for axis in axes.iter(){
            match axis {
                Axis::X => home_cmd.push_str("X "),
                Axis::Y => home_cmd.push_str("Y "),
                Axis::Z => home_cmd.push_str("Z "),
                _ => {}
            };
        };

        home_cmd.pop();
        
        vec!["M17".to_owned(), home_cmd]
    }

    fn get_reset_line_no_cmd(&self, line_no: u32) -> String {
        return format!("M110 N{}", line_no);
    }

    fn get_stop_cmd(&self, emergency: bool) -> String {
        if emergency {"M112".to_string()}  else {"M108".to_string()}
    }

    fn get_set_temperature_cmds(&self, new_t: &TemperatureTarget) -> Vec<String> {
        let code = match new_t.to_set {
            ProbePoint::HOTEND => {"M104"}
            ProbePoint::BED => {"M140"}
            ProbePoint::CHAMBER => {"M141"}
            ProbePoint::COOLER => {"M143"}
            _ => {
                error!("Cannot set temperature for {:?}", new_t.to_set);
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
        let mut ret_str = format!("N{} {}", line_no, cmd);
        let checksum: u8 = ret_str.as_bytes().iter().fold(0 as u8,|acc, x| acc ^ x );

        ret_str.push('*');
        ret_str.push_str(&checksum.to_string());
        ret_str
    }

    fn get_fan_speed_cmd(&self, index:u32, speed: f64) -> String {
        if speed <= 0. {
            format!("M107 P{}", index).to_string()
        } else {
            format!("M106 P{} S{}", index, std::cmp::min((speed * 255.) as u32, 255)).to_string()
        }
    }

    fn get_save_position_cmd(&self) -> String {
        return "G60".to_string();
    }

    fn get_restore_position_cmd(&self) -> String {
        return "G61 X Y Z".to_string();
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
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:0., current:22.58, target:0.0},
            Temperature{measured_from:ProbePoint::BED, index:0, power:0., current:23.11, target:70.0}], None));

        let test_line = " T:22.67 /66.66 B:23.11 /70.00 @:55 B@:127 W:30  ";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:(55./127.), current:22.67, target:66.66},
            Temperature{measured_from:ProbePoint::BED, index:0, power:1., current:23.11, target:70.0}], Some(30)));
    }

    #[test]
    fn parse_temperature_line_no_bed_no_residency() {
        let test_line = "T:22.67 /66.66 @:55";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::TEMPERATURE(vec![Temperature{measured_from:ProbePoint::HOTEND, index:0, power:(55./127.), current:22.67, target:66.66}], None));
    }

    #[test]
    fn parse_position_line() {
        let test_line = "X:0.13 Y:152.00 Z:3.01 E:-3.95 Count X:0 Y:12160 Z:6060";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::POSITION(Position{x: 0.13, y: 152.00, z: 3.01, e: -3.95}));
    }

    #[test]
    fn parse_send_error_line() {
        let test_lines = ["Error:Line Number is not Last Line Number+1, Last Line: 1", "Resend: 2", "ok"];
        
        assert_eq!(Marlin{}.parse_rx_line(test_lines[0]).unwrap(), Response::NACK(2));
        assert_eq!(Marlin{}.parse_rx_line(test_lines[1]).unwrap(), Response::NONE);
        assert_eq!(Marlin{}.parse_rx_line(test_lines[2]).unwrap(), Response::OK);
    }

    #[test]
    fn add_message_frame() {
        let test_line = "G1 X96.388 Y84.487 E0.04474";
        assert_eq!( Marlin{}.add_message_frame(1, test_line), "N1 G1 X96.388 Y84.487 E0.04474*107");
    }

    #[test]
    fn parse_busy_line() {
        let test_line = "echo:busy: processing";
        let resp = Marlin{}.parse_rx_line(test_line);
        assert_eq!(resp.unwrap(), Response::BUSY);
    }

    #[test]
    fn parse_fan_speed_no_idx() {
        let test_line = "M106 S255";
        let (idx, speed) = Marlin{}.parse_fan_speed(test_line);

        assert_eq!(idx, 0);
        assert_eq!(speed, 1.);
    }

    #[test]
    fn parse_fan_speed_with_idx() {
        let test_line = "M106 P2 S0";
        let (idx, speed) = Marlin{}.parse_fan_speed(test_line);

        assert_eq!(idx, 2);
        assert_eq!(speed, 0.);
    }

    #[test]
    fn parse_home_cmd() {
        let mut test_line = "G28 X Z";
        assert_eq!(Marlin{}.parse_home_cmd(test_line), enum_set!(Axis::Z | Axis::X));

        test_line = "G28 0";
        assert_eq!(Marlin{}.parse_home_cmd(test_line), enum_set!(Axis::X | Axis::Y | Axis::Z));

        test_line = "G28 ";
        assert_eq!(Marlin{}.parse_home_cmd(test_line), enum_set!(Axis::X | Axis::Y | Axis::Z));

        test_line = "G28 Y";
        assert_eq!(Marlin{}.parse_home_cmd(test_line), enum_set!(Axis::Y));
    }

    #[test]
    fn home_cmds() {
        assert_eq!(Marlin{}.get_home_cmds(&(Axis::X | Axis::Y | Axis::Z))[1], "G28 X Y Z");
        assert_eq!(Marlin{}.get_home_cmds(&(Axis::Y | Axis::Z))[1], "G28 Y Z");
        assert_eq!(Marlin{}.get_home_cmds(&(Axis::Y | Axis::Z | Axis::E))[1], "G28 Y Z");
    }

}