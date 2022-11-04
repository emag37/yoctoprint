use std::path::PathBuf;

use crossbeam::channel::{Sender, Receiver};
use rocket::{State,Data};
use serde::Deserialize;
use crate::internal_api;
use internal_api::*;
use rocket_contrib::json::Json;

struct InternalComms {
    to_internal: Sender<PrinterCommand>,
    from_internal: Receiver<PrinterResponse>
}

const OK_STR: &str = "OK!";

struct DataDir(PathBuf);

#[get("/status")]
fn status(comms: &State<InternalComms>) -> String {
    if let Err(e) = comms.to_internal.send(PrinterCommand::GetStatus) {
        return e.to_string();
    }
    
    match comms.from_internal.recv() {
        Ok(resp) => {
            match resp {
                PrinterResponse::Status(Ok(status)) => { serde_json::to_string(&status).unwrap() }
                PrinterResponse::Status(Err(e)) => {e.to_string()}
                _ => {"Unexpected response type".to_owned()}
            }
        }
        Err(e) => {
            return e.to_string();
        }
    }
}

#[post("/home")]
fn home(comms: &State<InternalComms>) -> String {
    if let Err(e) = comms.to_internal.send(PrinterCommand::Home) {
        return e.to_string();
    }
    
    match comms.from_internal.recv() {
        Ok(resp) => {
            match resp {
                PrinterResponse::GenericResult(Ok(())) => { OK_STR.to_owned() }
                PrinterResponse::GenericResult(Err(e)) => {e.to_string()}
                _ => {"Unexpected response type".to_owned()}
            }
        }
        Err(e) => {
            return e.to_string();
        }
    }
}

#[derive(Deserialize)]
struct RelativeCoords {
    x : Option<f64>,
    y : Option<f64>,
    z : Option<f64>,
    e : Option<f64>
}

#[post("/move", format = "application/json", data = "<relative_coords>")]
fn move_rel(comms: &State<InternalComms>, relative_coords :Json<RelativeCoords>) -> String {
    if let Err(e) = comms.to_internal.send(
        PrinterCommand::ManualMove(internal_api::Position{x: relative_coords.x.unwrap_or(0.0), 
        y: relative_coords.y.unwrap_or(0.0), 
        z: relative_coords.z.unwrap_or(0.0), 
        e: relative_coords.unwrap_or(0.0)})) {
        return e.to_string();
    }

    match comms.from_internal.recv() {
        Ok(resp) => {
            match resp {
                PrinterResponse::GenericResult(Ok(())) => { OK_STR.to_owned() }
                PrinterResponse::GenericResult(Err(e)) => {e.to_string()}
                _ => {"Unexpected response type".to_owned()}
            }
        }
        Err(e) => {
            return e.to_string();
        }
    }
}

#[put("/upload_gcode?filename&<name>", format = "plain", data = "<data>")]
fn upload(data: Data, filename: String, data_dir: DataDir) -> Result<String, std::io::Error> {
    
}

pub fn run_api(to_internal: Sender<PrinterCommand>, from_internal: Receiver<PrinterResponse>, data_dir: PathBuf) {
    let api_rocket = rocket::build()
    .mount("/api", routes![status, home])
    .manage(InternalComms{to_internal: to_internal, from_internal:from_internal})
    .manage(DataDir{0:data_dir});

    return rocket::execute(async move {
        api_rocket.launch().await;
    });
}