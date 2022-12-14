use std::io::{Error, ErrorKind};
use std::path::PathBuf;

use crossbeam::channel::{Sender, Receiver, RecvError};
use rocket::data::ByteUnit;
use rocket::{State,Data, Request};
use crate::internal_api;
use crate::file;
use internal_api::*;
use rocket::serde::json::{Json};
use rocket::serde::{Serialize,Deserialize};
use rocket_cors::CorsOptions;
use enumset::EnumSet;
use rocket::fs::{NamedFile};

struct InternalComms {
    to_internal: Sender<PrinterCommand>,
    from_internal: Receiver<PrinterResponse>
}

struct WebUiDir(PathBuf);

type DataDir = PathBuf;

struct ApiError(std::io::Error);

impl From<std::io::Error> for ApiError{
    fn from(err: std::io::Error) -> Self {
        ApiError(err)
    }
}

fn crossbeam_err_to_io_err<T>(crossbeam_err: T) -> ApiError 
where T: std::error::Error + std::marker::Sync + std::marker::Send +'static{
    ApiError(Error::new(ErrorKind::BrokenPipe, crossbeam_err))
}

fn resp_generic_result_or_err(crossbeam_result: Result<PrinterResponse, RecvError>) -> Result<(), ApiError> {
    match crossbeam_result {
            Err(e) => {Err(crossbeam_err_to_io_err(e))}
            Ok(r) => {
                match r {
                    PrinterResponse::GenericResult(Ok(())) => { Ok(())}
                    PrinterResponse::GenericResult(Err(e)) => {Err(ApiError(Error::new(ErrorKind::NotFound, e)))}
                    _ => {Err(ApiError(Error::new(ErrorKind::Unsupported, "Unexpected response")))}
                }
            }
        }
}

#[derive(Debug, Deserialize, Clone)]
struct ConnectParams {
    pub port : String,
    pub baud : u32
}

#[post("/connect", format = "application/json", data = "<params>")]
fn connect(comms: &State<InternalComms>, params: Json<ConnectParams>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::Connect(params.port.clone().into(), params.baud)) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[get("/status")]
fn status(comms: &State<InternalComms>) -> Result<Json<PrinterStatus>, ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::GetStatus) {
        return Err(crossbeam_err_to_io_err(e));
    }
    
    match comms.from_internal.recv() {
        Ok(resp) => {
            match resp {
                PrinterResponse::Status(Ok(status)) => { Ok(Json(status)) }
                PrinterResponse::GenericResult(Err(e)) | PrinterResponse::Status(Err(e)) => {Err(ApiError(e))}
                _ => {Err(ApiError(Error::new(ErrorKind::Unsupported, format!("Unexpected response"))))}
            }
        }
        Err(e) => {
            Err(crossbeam_err_to_io_err(e))
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct HomeAxes {
    pub axes : Vec<String>
}

#[post("/home", format = "application/json", data = "<home_axes>")]
fn home(comms: &State<InternalComms>, home_axes : Json<HomeAxes>) -> Result<(), ApiError> {
    let mut internal_axes : EnumSet<internal_api::Axis> = EnumSet::new();

    for axis in home_axes.axes.iter() {
        match axis.to_uppercase().trim() {
            "X" => {internal_axes |= internal_api::Axis::X},
            "Y" => {internal_axes |= internal_api::Axis::Y},
            "Z" => {internal_axes |= internal_api::Axis::Z},
            "ALL" => {internal_axes |= internal_api::Axis::X | internal_api::Axis::Y | internal_api::Axis::Z},
            _ => {}
        }
    }

    if let Err(e) = comms.to_internal.send(PrinterCommand::Home(internal_axes)) {
        return Err(crossbeam_err_to_io_err(e));
    }
    
    resp_generic_result_or_err(comms.from_internal.recv())
}

#[derive(Debug, Deserialize, Clone)]
struct RelativeCoords {
    x : Option<f64>,
    y : Option<f64>,
    z : Option<f64>,
    e : Option<f64>
}

#[post("/move", format = "application/json", data = "<relative_coords>")]
fn move_rel(comms: &State<InternalComms>, relative_coords : Json<RelativeCoords>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(
        PrinterCommand::ManualMove(internal_api::Position{x: relative_coords.x.unwrap_or(0.0), 
        y: relative_coords.y.unwrap_or(0.0), 
        z: relative_coords.z.unwrap_or(0.0), 
        e: relative_coords.e.unwrap_or(0.0)})) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[derive(Debug, Deserialize, Clone)]
struct FanSpeed {
    index : Option<u32>,
    speed: f64
}
#[post("/set_fan_speed", format = "application/json", data = "<fan_speed>")]
fn set_fan_speed(comms: &State<InternalComms>, fan_speed : Json<FanSpeed>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::SetFanSpeed((
        fan_speed.index.unwrap_or(0),
        fan_speed.speed
    ))) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[put("/upload_gcode?<filename>", format="application/octet-stream", data = "<data>")]
async fn upload_gcode(data: Data<'_>, filename: String, data_dir: &State<DataDir>) -> Result<(), ApiError> {
    let size_limit: ByteUnit = "50 MB".parse().unwrap();

    let mut full_path : PathBuf = data_dir.clone().to_path_buf();
    full_path.push(file::GCODE_DIR);
    full_path.push(filename);
    
    if std::path::Path::exists(full_path.as_path()) {
        return Err(ApiError(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("file already exists @ {:?}", full_path))));
    }
    let stream = data.open(size_limit);
    let file = stream.into_file(full_path.as_path()).await?;
    
    if !file.is_complete() {
        return Err(ApiError(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Unable to write entire file")));
    }
    
    Ok(())
}

#[derive(Debug, Serialize, Clone)]
struct ApiFileInfo {
    pub last_modified_secs_since_epoch: u64,
    pub name: String,
    pub size: u64
}
#[derive(Debug, Serialize, Clone)]
struct FileList {
    pub files: Vec<ApiFileInfo>
}
#[get("/list_gcode")]
fn list_gcode(data_dir: &State<DataDir>) -> Json<FileList> {
    let api_files : Vec<ApiFileInfo> = file::find_gcode_files(data_dir).unwrap().iter()
    .map(|file| {
        ApiFileInfo{name:
        file.path.file_name().unwrap().to_str().unwrap().to_string(),
        size: file.size,
        last_modified_secs_since_epoch: file.last_modified_since_epoch.as_secs()}
    }).collect();

    return Json(FileList{files:api_files});
}

#[post("/set_gcode?<filename>")]
fn set_gcode(comms: &State<InternalComms>, filename: String) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::SetGcodeFile(PathBuf::from(filename))) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[delete("/delete_gcode?<filename>")]
fn delete_gcode(comms: &State<InternalComms>, filename: String) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::DeleteGcodeFile(PathBuf::from(filename))) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[post("/start_print")]
fn start_print(comms: &State<InternalComms>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::StartPrint) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[post("/stop_print")]
fn stop_print(comms: &State<InternalComms>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::StopPrint) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[post("/pause_print")]
fn pause_print(comms: &State<InternalComms>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::PausePrint) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[post("/set_temperature", format = "application/json", data = "<temperature>")]
fn set_temperature(comms: &State<InternalComms>, temperature : Json<TemperatureTarget>) -> Result<(), ApiError> {
    if let Err(e) = comms.to_internal.send(PrinterCommand::SetTemperature(*temperature)) {
        return Err(crossbeam_err_to_io_err(e));
    }

    resp_generic_result_or_err(comms.from_internal.recv())
}

#[get("/")]
async fn index(webui_dir: &State<WebUiDir>) -> std::option::Option<NamedFile> {
    info!("Current dir: {:?}. Index: {:?}", std::env::current_dir(), webui_dir.0.as_path().join("index.html"));
    NamedFile::open(webui_dir.0.as_path().join("index.html")).await.ok()
}

#[get("/<file..>")]
async fn serve_file(file: PathBuf, webui_dir: &State<WebUiDir>) -> Option<NamedFile> {
    NamedFile::open(webui_dir.0.as_path().join(file)).await.ok()
}

impl<'r> rocket::response::Responder<'r, 'static> for ApiError {
    fn respond_to(self, _request: &Request<'_>) -> rocket::response::Result<'static> {
        let error_str = format!("Error: {:?}\nDescription: {:?}", self.0.kind(), self.0);

        Ok(rocket::Response::build().
        header(rocket::http::ContentType::HTML).
        status(rocket::http::Status::InternalServerError)
        .sized_body(error_str.len(), std::io::Cursor::new(error_str))
        .finalize())
    }
}

pub fn run_api(to_internal: Sender<PrinterCommand>, from_internal: Receiver<PrinterResponse>, data_dir: PathBuf, webui_dir: PathBuf) {
    let cors = CorsOptions::default().to_cors().unwrap();

    let api_rocket = rocket::build()
    .mount("/api", routes![connect, status, home, move_rel, upload_gcode, list_gcode, set_gcode, delete_gcode, start_print, stop_print, pause_print, set_temperature, set_fan_speed])
    .mount("/", routes![index, serve_file])
    .manage(InternalComms{to_internal: to_internal, from_internal:from_internal})
    .manage(data_dir as DataDir)
    .manage(WebUiDir(webui_dir))
    .attach(cors);

    
    return rocket::execute(async move {
        let _rocket = api_rocket.launch().await.expect("Error launching REST API");
    });
}