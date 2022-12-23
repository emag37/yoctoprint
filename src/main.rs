use std::path::{PathBuf};
use log::{debug, info, error, warn};
use std::io::{Error,ErrorKind};
use crate::printer::{Printer, SimulatedPrinter, PrinterControl};
use crate::internal_api::*;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate rocket;

mod serial;
mod file;
mod rest_api;
mod internal_api;
mod printer;
mod marlin;
mod interval_timer;

fn handle_incoming_cmd(printer: &mut Option<Box<dyn PrinterControl>>, cmd: &internal_api::PrinterCommand, base_path: &PathBuf) -> internal_api::PrinterResponse{
    if printer.is_none() {
        match cmd {
            PrinterCommand::GetStatus => {
                return internal_api::PrinterResponse::Status(Ok(internal_api::PrinterStatus::default()))
            }
            PrinterCommand::Connect(path, baud)=> {
                let path_str = path.to_str().unwrap();
    
                if path_str == "sim" {
                    *printer = Some(Box::new(SimulatedPrinter::new()));
                    return internal_api::PrinterResponse::GenericResult(Ok(()))
                } else {
                match serial::PrinterComms::new(path_str, *baud) {
                    Ok(p) => {
                        println!("Will connect printer @: {} baud: {}", path_str, baud);
                            match Printer::new(p) {
                                Ok(p) => {
                                    *printer = Some(Box::new(p));
                                    return internal_api::PrinterResponse::GenericResult(Ok(()))
                                }
                                Err(e) => {return internal_api::PrinterResponse::GenericResult(Err(e));}
                            }
                    }
                    Err(e) => {
                        println!("Error connecting printer @ {}, baud {}. {}", path_str, baud, e);
                        return PrinterResponse::GenericResult(Err(e));
                        }
                    }
                }
            }
            _ => {
                return PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::NotFound, "No printer connected")));
            }
        }
    }

    let printer_ref = printer.as_mut().unwrap();

    match cmd {
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
        PrinterCommand::Home(axes) => {
            return internal_api::PrinterResponse::GenericResult(printer_ref.go_home(axes));
        },
        PrinterCommand::Connect(_, _) => {
            return PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::AlreadyExists, "Printer already connected, disconnect it first")));
        }
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
    let mut scan_timer = interval_timer::IntervalTimer::new(std::time::Duration::from_secs(2));
    let mut printer : Option<Box<dyn PrinterControl>> = None;

    env_logger::init();
    info!("Yoctoprint starting!");

    let base_dir = init_base_dir().unwrap();
    init_gcode_dir(&base_dir).unwrap();

    let (they_send, we_recv) = crossbeam::channel::unbounded();
    let (we_send, they_recv) = crossbeam::channel::unbounded::<PrinterResponse>();

    let base_dir_api = base_dir.clone();
    let _api = std::thread::spawn( ||{
        rest_api::run_api(they_send, they_recv, base_dir_api);
    });

    loop {
        if let Ok(new_msg) =  we_recv.try_recv() {
            let resp = handle_incoming_cmd(&mut printer, &new_msg, &base_dir);

            we_send.send(resp).expect("Error sending response to external API");
        }

        if let Some(ref mut cur_printer) = printer {
            if cur_printer.get_state() == PrintState::STARTED {
                cur_printer.print_next_line();
            } else {
                cur_printer.poll_new_status();
            }
        } else if scan_timer.check() {
            info!("Looking for printer...");
            if let Ok(found) = serial::find_printer() {
                info!("Found printer with capabilities: {:?}", found.fw_info);
                match Printer::new(found) {
                    Ok(p) => {
                        printer = Some(Box::new(p))
                    }
                    Err(e) => {
                        error!("Got error connecting printer: {:?}", e);
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
