use std::path::{PathBuf};
use std::io::{Error,ErrorKind};
use crate::printer::Printer;
use crate::internal_api::*;
#[cfg(feature = "simulated_printer")]
use rand;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate rocket;

mod serial;
mod file;
mod rest_api;
mod internal_api;
mod printer;
mod marlin;


#[cfg(feature = "simulated_printer")]
fn handle_incoming_cmd(cur_status : &mut internal_api::PrinterStatus, cmd: &internal_api::PrinterCommand, base_path: &PathBuf) -> internal_api::PrinterResponse{
    use std::str::FromStr;

    match cmd {
        PrinterCommand::GetStatus => {
            return internal_api::PrinterResponse::Status(Ok(cur_status.clone()));
        },
        PrinterCommand::DeleteGcodeFile(path) => {
            internal_api::PrinterResponse::GenericResult(std::fs::remove_file(file::get_abs_gcode_path(base_path, path)))
        },
        PrinterCommand::SetGcodeFile(path) => {
            let total_lines = rand::random::<u32>() % 500000;
            let cur_line = rand::random::<u32>() % total_lines;
            cur_status.gcode_lines_done_total = Some((path.to_str().unwrap().to_string(), cur_line, total_lines));
            internal_api::PrinterResponse::GenericResult(Ok(()))
        },
        _ => return internal_api::PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::NotFound, "We haven't simulated that yet")))
    }
}

#[cfg(not(feature = "simulated_printer"))]
fn handle_incoming_cmd(printer: &mut Option<Printer>, cmd: &internal_api::PrinterCommand, base_path: &PathBuf) -> internal_api::PrinterResponse{
    if printer.is_none() {
        match cmd {
            PrinterCommand::GetStatus => {
                return internal_api::PrinterResponse::Status(Ok(internal_api::PrinterStatus::default()))
            }
            PrinterCommand::Connect(_,_) => {},
            _ => {
                return PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::NotFound, "No printer connected")));
            }
        }
    }

    let printer_ref = printer.as_mut().unwrap();

    match cmd {
        PrinterCommand::Connect(path, baud)=> {
            if printer.is_some() {
                return PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::AlreadyExists, "Printer already connected, disconnect it first")));
            }

            let path_str = path.to_str().unwrap();
            match serial::PrinterComms::new(path_str, *baud) {
                Ok(p) => {
                    println!("Will connect printer @: {} baud: {}", path_str, baud);
                    *printer = Printer::new(p);
                    return internal_api::PrinterResponse::GenericResult(Ok(()));
                }
                Err(e) => {
                    println!("Error connecting printer @ {}, baud {}. {}", path_str, baud, e);
                    return PrinterResponse::GenericResult(Err(e));
                }
            }
        }
        PrinterCommand::Disconnect => {
            *printer = None;
            return internal_api::PrinterResponse::GenericResult(Ok(()));

        },
        PrinterCommand::SetGcodeFile(path) => {
            internal_api::PrinterResponse::GenericResult(printer_ref.set_gcode_file(&file::get_abs_gcode_path(base_path, path)))
        },
        PrinterCommand::DeleteGcodeFile(path) => {
            internal_api::PrinterResponse::GenericResult(std::fs::remove_file(file::get_abs_gcode_path(base_path, path)))
        },
        PrinterCommand::StartPrint => {
            internal_api::PrinterResponse::GenericResult(printer_ref.start())
        },
        PrinterCommand::PausePrint => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.pause());
        }
        PrinterCommand::StopPrint => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.stop());
        },
        PrinterCommand::GetStatus => {
            return internal_api::PrinterResponse::Status(printer_ref.get_status());
        },
        PrinterCommand::ManualMove(rel_pos) => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.move_relative(rel_pos));
        },
        PrinterCommand::SetTemperature(new_temp) => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.set_temperature(new_temp));
        },
        PrinterCommand::Home => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.go_home());
        },
    }
}

fn init_base_dir() -> std::io::Result<PathBuf> {
    let mut base_dir : PathBuf = match dirs::home_dir() {
        Some(home) => {home}
        None => {return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cannot find home directory for user"));}
    };
    
    base_dir.push(".yoctoprint");
    match std::fs::create_dir(&base_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {Ok(base_dir)}
        Err(e) => {Err(e)}
        Ok(()) => {Ok(base_dir)}
    }
}

fn init_gcode_dir(base_dir: &PathBuf) -> std::io::Result<PathBuf> {
    let mut gcode_dir = base_dir.clone();
    gcode_dir.push(file::GCODE_DIR);
    match std::fs::create_dir(&gcode_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {Ok(gcode_dir)}
        Err(e) => {Err(e)}
        Ok(()) => {Ok(gcode_dir)}
    }
}

fn main() {
    #[cfg(feature = "simulated_printer")]
    let mut sim_status = internal_api::PrinterStatus{ 
        connected: true,
        manual_control_enabled: false, 
        state: internal_api::PrintState::CONNECTED, 
        temperatures: vec![
            internal_api::Temperature{measured_from: internal_api::ProbePoint::HOTEND, index: 0,
            power: 128,
            current: 75.6,
            target: 220.0},
            internal_api::Temperature{measured_from: internal_api::ProbePoint::BED, index: 0,
                power: 128,
                current: 23.4,
                target: 70.0}
        ], 
        position: internal_api::Position{x:10.0, y:1.2, z:20.5, e: 100.0}, 
        gcode_lines_done_total: Some(("sheriff_woody.gcode".to_string(), 102034, 750876))
    };

    let mut printer : Option<Printer> = None;
    let base_dir = init_base_dir().unwrap();
    init_gcode_dir(&base_dir).unwrap();

    let (they_send, we_recv) = crossbeam::channel::unbounded();
    let (we_send, they_recv) = crossbeam::channel::unbounded::<PrinterResponse>();

    let base_dir_api = base_dir.clone();
    let _api = std::thread::spawn( ||{
        rest_api::run_api(they_send, they_recv, base_dir_api);
    });
    
    loop {
        if let Ok(new_msg) =  we_recv.recv_timeout(std::time::Duration::from_millis(5)) {
            #[cfg(feature = "simulated_printer")]
            let resp = handle_incoming_cmd(&mut sim_status, &new_msg, &base_dir);
            #[cfg(not(feature = "simulated_printer"))]
            let resp = handle_incoming_cmd(&mut printer, &new_msg, &base_dir);

            we_send.send(resp).expect("Error sending response to external API");
        }

        if let Some(ref mut cur_printer) = printer {
            if cur_printer.get_state() == PrintState::STARTED {
                cur_printer.print_next_line();
            } else {
                cur_printer.poll_new_status();
            }
        } else {
            if let Ok(found) = serial::find_printer() {
                println!("Found printer with capabilities: {:?}", found.fw_info);
                printer = Printer::new(found);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

}
