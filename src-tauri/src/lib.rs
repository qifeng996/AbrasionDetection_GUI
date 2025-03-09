// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod sqlite;
mod serial;

use std::convert::Infallible;
use std::ops::Index;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_serial::{DataBits, Parity, SerialPort, SerialPortBuilderExt, SerialStream, StopBits,SerialPortType};
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, MutexGuard, TryLockError};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tauri::path;
use crate::serial::{get_port, set_serial_cfg, stop_serial_task};

pub struct AppWrapper {
    pub app_handler: AppHandle,
    serial_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    // pub port: SerialStream,
}
#[derive(Clone,serde::Serialize)]
pub struct PortInfo {
    pub port: String,
    pub info: String,
}
#[derive(Clone, serde::Serialize)]
struct Payload {
    data: Vec<i32>,
}
#[derive(Clone, serde::Serialize)]
struct SerialPortList {
    port_vec: Vec<PortInfo>,
}
pub struct StateWrapper(pub Mutex<AppWrapper>);


#[tokio::main]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    tauri::Builder::default().setup(|app| {
        let app_wrapper = StateWrapper(Mutex::new(AppWrapper {
            app_handler: app.handle().clone(),
            serial_task: Arc::new(Default::default()),
        }));
        app.manage(app_wrapper);
        Ok(())
    })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_port,set_serial_cfg,stop_serial_task])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
