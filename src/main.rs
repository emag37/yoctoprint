#![feature(noop_waker)]

use std::path::{PathBuf};
use log::{debug, info, error, warn};
use std::io::ErrorKind;
use std::fs::File;
use crate::printer::{Printer, SimulatedPrinter, PrinterControl};
use crate::internal_api::*;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate rocket;
use clap::Parser;
use daemonize::Daemonize;

mod serial;
mod file;
mod rest_api;
mod internal_api;
mod printer;
mod marlin;
mod interval_timer;
mod recv_channel_async_wrapper;

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
                        info!("Will connect printer @: {} baud: {}", path_str, baud);
                            match Printer::new(p) {
                                Ok(p) => {
                                    *printer = Some(Box::new(p));
                                    return internal_api::PrinterResponse::GenericResult(Ok(()))
                                }
                                Err(e) => {return internal_api::PrinterResponse::GenericResult(Err(e));}
                            }
                    }
                    Err(e) => {
                        info!("Error connecting printer @ {}, baud {}. {}", path_str, baud, e);
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
            match printer_ref.get_status() {
                Ok(status) => {
                    match status.gcode_lines_done_total {
                        Some(gcode) => {
                            if gcode.0.ends_with(path.as_os_str().to_str().unwrap()) {
                                if let Err(e) = printer_ref.clear_gcode_file() {
                                    return internal_api::PrinterResponse::GenericResult(Err(e));
                                }
                            }
                        },
                        None => {}
                    }
                }
                _ => {}
            }

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
        PrinterCommand::SetFanSpeed(new_speed) => {
            return PrinterResponse::GenericResult(printer_ref.set_fan_speed(new_speed));
        }
        PrinterCommand::OpenConsole => {
            return PrinterResponse::ConsoleChannel(printer_ref.create_external_console());
        },
        PrinterCommand::GetPrinterInfo => {
            return PrinterResponse::Info(printer_ref.get_info());
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


const DEFAULT_WEBUI_DIR : &str = "./ui/dist";

#[derive(Parser, Debug)]
#[command()]
struct Args {
    /// Directory containing WebUI
    #[arg(long, default_value=DEFAULT_WEBUI_DIR)]
    web_ui: String,

    // Log level
    #[arg(short, long, default_value="info")]
    log_level: String,

    // Run as a Daemon (fork to background and then exit)
    #[arg(long)]
    daemon: bool,

    // If running as a deamon, create this PID file
    #[arg(long)]
    pid_file: Option<PathBuf>
}

fn main() {
    let mut scan_timer = interval_timer::IntervalTimer::new(std::time::Duration::from_secs(2));
    let mut printer : Option<Box<dyn PrinterControl>> = None;

    let args = Args::parse();

    let log_level = match args.log_level.to_lowercase().as_str() {
        "error" => log::Level::Error,
        "warn" => log::Level::Warn,
        "info" => log::Level::Info,
        "debug" => log::Level::Debug,
        "trace" => log::Level::Trace,
        invalid => {error!("Invalid log level: {}, default to info", invalid); log::Level::Info}
    };

    simple_logger::init_with_level(log_level).unwrap();
    info!("Yoctoprint starting!");

    let base_dir = init_base_dir().unwrap();
    if args.daemon {
        let stdout = File::create(base_dir.join("daemon.log")).unwrap();
        let stderr = File::create(base_dir.join("daemon.err")).unwrap();
    
        let daemonize = Daemonize::new()
            .pid_file(args.pid_file.unwrap_or(base_dir.join("daemon.pid"))) // Every method except `new` and `start`
            .working_directory(base_dir.clone()) // for default behaviour.
            .stdout(stdout)  // Redirect stdout to `/tmp/daemon.out`.
            .stderr(stderr);  // Redirect stderr to `/tmp/daemon.err`.;
    
        match daemonize.start() {
            Ok(_) => info!("Success, daemonized"),
            Err(e) => error!("Error, {}", e),
        }
    }

    init_gcode_dir(&base_dir).unwrap();

    let (they_send, we_recv) = crossbeam::channel::unbounded();
    let (we_send, they_recv) = crossbeam::channel::unbounded::<PrinterResponse>();

    let base_dir_api = base_dir.clone();
    let _api = std::thread::spawn( ||{
        rest_api::run_api(they_send, they_recv, base_dir_api, args.web_ui.into());
    });

    let ctrl_c_pressed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let ctrl_c_pressed_clone  = ctrl_c_pressed.clone();

    if let Err(e) = ctrlc::set_handler(move || {
        warn!("Shutting down due to Ctrl+C pressed.");
        ctrl_c_pressed_clone.store(true, std::sync::atomic::Ordering::Relaxed);
    }) {
        error!("Failed to set Ctrl+C handler: {:?}", e);
    }

    while !ctrl_c_pressed.load(std::sync::atomic::Ordering::Relaxed) {
        if let Ok(new_msg) =  we_recv.try_recv() {
            let resp = handle_incoming_cmd(&mut printer, &new_msg, &base_dir);

            we_send.send(resp).expect("Error sending response to external API");
        }

        if let Some(ref mut cur_printer) = printer {
           cur_printer.next_action().expect("Error performing next printer action!");
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
