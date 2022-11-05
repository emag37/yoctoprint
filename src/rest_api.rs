use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;

use crossbeam::channel::{Sender, Receiver};
use rocket::data::ByteUnit;
use rocket::{State,Data};
use serde::Serialize;
use crate::internal_api;
use crate::file;
use internal_api::*;
use rocket::serde::json::{Json, Value, json, self};
use rocket::serde::{Deserialize};
use std::fs::File;

struct InternalComms {
    to_internal: Sender<PrinterCommand>,
    from_internal: Receiver<PrinterResponse>
}

const OK_STR: &str = "OK!";

type DataDir = PathBuf;

#[get("/status")]
fn status(comms: &State<InternalComms>) -> String {
    if let Err(e) = comms.to_internal.send(PrinterCommand::GetStatus) {
        return e.to_string();
    }
    
    match comms.from_internal.recv() {
        Ok(resp) => {
            match resp {
                PrinterResponse::Status(Ok(status)) => { serde_json::to_string(&status).unwrap() }
                PrinterResponse::GenericResult(Err(e)) | PrinterResponse::Status(Err(e)) => {e.to_string()}
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

#[derive(Debug, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
pub struct RelativeCoords {
    x : Option<f64>,
    y : Option<f64>,
    z : Option<f64>,
    e : Option<f64>
}

#[post("/move", format = "application/json", data = "<relative_coords>")]
fn move_rel(comms: &State<InternalComms>, relative_coords : rocket::serde::json::Json<RelativeCoords>) -> String {
    if let Err(e) = comms.to_internal.send(
        PrinterCommand::ManualMove(internal_api::Position{x: relative_coords.x.unwrap_or(0.0), 
        y: relative_coords.y.unwrap_or(0.0), 
        z: relative_coords.z.unwrap_or(0.0), 
        e: relative_coords.e.unwrap_or(0.0)})) {
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

#[put("/upload_gcode?<filename>", format="application/octet-stream", data = "<data>")]
async fn upload_gcode(data: Data<'_>, filename: String, data_dir: &State<DataDir>) -> Result<String, std::io::Error> {
    let size_limit: ByteUnit = "50 MB".parse().unwrap();

    let mut fullPath : PathBuf = data_dir.clone().to_path_buf();
    fullPath.push(file::GCODE_DIR);
    fullPath.push(filename);
    
    if std::path::Path::exists(fullPath.as_path()) {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("file already exists @ {:?}", fullPath)));
    }
    let stream = data.open(size_limit);
    let file = stream.into_file(fullPath.as_path()).await?;
    
    if !file.is_complete() {
        return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Unable to write entire file"));
    }
    
    Ok(OK_STR.to_owned())
}
#[derive(Debug, Serialize, Clone)]
#[serde(crate = "rocket::serde")]
struct FileList {
    files: Vec<String>
}
#[get("/list_gcode")]
fn list_gcode(data_dir: &State<DataDir>) -> Json<FileList> {
    let mut trimmed_files : Vec<String> = Vec::new();
    let files: Vec<PathBuf> = file::find_gcode_files(data_dir).unwrap();


    for file in files {
        let trimmed : _= file.iter()
        .skip_while(|component| data_dir.iter().any(|dc| dc == *component))
        .collect::<PathBuf>();
        trimmed_files.push(trimmed.to_str().unwrap().to_owned());
    }
    return Json(FileList{files:trimmed_files});
}

pub fn run_api(to_internal: Sender<PrinterCommand>, from_internal: Receiver<PrinterResponse>, data_dir: PathBuf) {
    let api_rocket = rocket::build()
    .mount("/api", routes![status, home, move_rel, upload_gcode, list_gcode])
    .manage(InternalComms{to_internal: to_internal, from_internal:from_internal})
    .manage(data_dir as DataDir);

    return rocket::execute(async move {
        api_rocket.launch().await;
    });
}