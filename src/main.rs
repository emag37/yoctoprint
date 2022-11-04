use std::path::PathBuf;
use std::io::{Error,ErrorKind};
use crate::printer::Printer;
use crate::internal_api::*;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate rocket;
#[macro_use] extern crate crossbeam;

mod serial;
mod file;
mod rest_api;
mod internal_api;
mod printer;
mod marlin;


fn handle_incoming_cmd(printer: &mut Option<Printer>, cmd: &internal_api::PrinterCommand) -> internal_api::PrinterResponse{
    if !matches!(cmd, PrinterCommand::Connect(_,_)) && printer.is_none() {
        return PrinterResponse::GenericResult(Err(std::io::Error::new(ErrorKind::NotFound, "No printer connected")));
    }

    let mut printer_ref = printer.as_mut().unwrap();

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
        PrinterCommand::StartPrint(gcode) => {
            return internal_api::PrinterResponse::GenericResult(match printer_ref.set_gcode_file(gcode.clone()) {
                Ok(()) => {printer_ref.start()}
                Err(e) => {Err(e)} 
            });
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
    match std::fs::create_dir(base_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {Ok(base_dir)}
        Err(e) => {Err(e)}
        Ok(()) => {Ok(base_dir)}
    }
}

fn init_gcode_dir(base_dir: &PathBuf) -> std::io::Result<PathBuf> {
    let mut gcode_dir = base_dir.clone();
    gcode_dir.push(file::GCODE_DIR);
    match std::fs::create_dir(gcode_dir) {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {Ok(gcode_dir)}
        Err(e) => {Err(e)}
        Ok(()) => {Ok(gcode_dir)}
    }
}

fn main() {
    let mut printer : Option<Printer> = None;
    let base_dir = init_base_dir().unwrap();
    let gcode_dir = init_gcode_dir(&base_dir).unwrap();

    let (they_send, we_recv) = crossbeam::channel::unbounded();
    let (we_send, they_recv) = crossbeam::channel::unbounded::<PrinterResponse>();

    let api = std::thread::spawn( ||{
        rest_api::run_api(they_send, they_recv, base_dir.clone());
    });
    
    if let Ok(found) = serial::find_printer() {
        println!("Found printer with capabilities: {:?}", found.fw_info);
        printer = Printer::new(found);
    }
    let gcodes : Vec<PathBuf> = file::find_gcode_files(std::path::Path::new("/home/ubuntu/yoctoprint")).unwrap();

    println!("Found gcode files: {}", &gcodes.iter().fold(String::new(), |mut ret: String, path| {
        ret = ret + path.to_str().unwrap() + ",";
        return ret;
    }));
    //println!("Let's print {}", gcode[0].display());

    //let mut to_print = file::GCodeFile::new(&gcodes[0]).unwrap();

    loop {
        if let Ok(new_msg) =  we_recv.try_recv() {
           let resp = handle_incoming_cmd(&mut printer, &new_msg);
           we_send.send(resp);
        }
        if let Some(ref mut cur_printer) = printer {
            if cur_printer.get_state() == PrintState::STARTED {
                cur_printer.print_next_line();
            } else {
                cur_printer.poll_new_status();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

   
    api.join();
}
