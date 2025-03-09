// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use std::convert::Infallible;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_serial::{DataBits, Parity, SerialPort, SerialPortBuilderExt, SerialStream, StopBits};
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, MutexGuard, TryLockError};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tauri::path;
pub struct AppWrapper {
    pub app_handler: AppHandle,
    serial_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    // pub port: SerialStream,
}
#[derive(Clone, serde::Serialize)]
struct Payload {
    data: Vec<i32>,
}
#[derive(Clone, serde::Serialize)]
struct SerialPortList {
    port_vec: Vec<String>,
}
pub struct StateWrapper(pub Mutex<AppWrapper>);
fn parse_data(received: &[u8]) -> Option<Vec<i32>> {
    if received.len() < 44 {
        return None;
    }

    let mut result = Vec::new();

    for i in 0..9 {
        let value = i32::from_le_bytes([
            received[4 + i * 4],
            received[5 + i * 4],
            received[6 + i * 4],
            received[7 + i * 4]
        ]);
        result.push(value);
    }

    Some(result)
}

#[tauri::command]
fn set_serial_cfg(
    state: tauri::State<StateWrapper>,
    port_name: &str,
    band_rate: u32,
    stop_bits: u32,
    data_bits: u32,
) -> Result<String, String> {
    // 尝试打开串口
    let mut port = match tokio_serial::new(port_name, band_rate).open_native_async() {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to open serial port: {}", e)),
    };

    // 设置校验位
    if let Err(e) = port.set_parity(Parity::None) {
        return Err(format!("Failed to set parity: {}", e));
    }

    // 设置数据位
    let data_bits = match data_bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        _ => return Err(format!("Invalid data bits: {}", data_bits)),
    };
    if let Err(e) = port.set_data_bits(data_bits) {
        return Err(format!("Failed to set data bits: {}", e));
    }

    // 设置停止位
    let stop_bits = match stop_bits {
        1 => StopBits::One,
        2 => StopBits::Two,
        _ => return Err(format!("Invalid stop bits: {}", stop_bits)),
    };
    if let Err(e) = port.set_stop_bits(stop_bits) {
        return Err(format!("Failed to set stop bits: {}", e));
    }

    let task = tokio::spawn({
        let app_handle = match state.0.try_lock() {
            Ok(locked_state) => locked_state.app_handler.clone(),
            Err(_) => return Err("Failed to acquire lock for app handler.".to_string()),
        };
        let mut port = port;
        async move {
            let mut buf = [0; 512];
            let mut data = Vec::new();
            let mut last_receive = tokio::time::Instant::now();
            loop {
                tokio::select! {
                    result = port.read(&mut buf) => {
                            match result {
                            Ok(n) if n > 0 => {
                                data.extend_from_slice(&buf[..n]);
                                last_receive = tokio::time::Instant::now();
                            }
                            Ok(_) => {}
                            Err(e) => eprintln!("读取错误: {}", e),
                        }
                    }
                    _ = sleep(Duration::from_millis(1)) => {
                        if last_receive.elapsed() >= Duration::from_millis(20) {
                            if !data.is_empty() {
                                println!("数据包接收完成: {:?}", data);
                                match parse_data(&data) {
                                    Some(result) => {
                                        let payload = Payload {
                                                data: result
                                            };
                                        app_handle.emit("data_received", payload).expect("Failed to emit event");
                                    }
                                    None => println!("数据长度不足 44 字节"),
                                }

                                data.clear();
                            }
                        }
                    }
                }
            }
        }
    });
    let serial_task = match state.0.try_lock() {
        Ok(locked_state) => locked_state.serial_task.clone(), // 先获取 Arc<Mutex<Option<JoinHandle<()>>>>，避免双重锁定
        _ => return Err("Failed to lock state.".to_string()),
    };

    let mut serial_task_guard = match serial_task.try_lock() {
        Ok(guard) => guard, // 获取 MutexGuard
        Err(_) => return Err("Failed to lock serial_task.".to_string()),
    };

    *serial_task_guard = Some(task); // 直接赋值
    // 串口配置成功
    Ok("Serial configuration successfully applied.".to_string())
}
#[tauri::command]
fn stop_serial_task(state: tauri::State<StateWrapper>) -> Result<String, String> {
    let serial_task = match state.0.try_lock() {
        Ok(locked_state) => locked_state.serial_task.clone(), // 先获取 Arc<Mutex<Option<JoinHandle<()>>>>，避免双重锁定
        _ => return Err("Failed to lock state.".to_string()),
    };

    let mut serial_task_guard = match serial_task.try_lock() {
        Ok(guard) => guard, // 获取 MutexGuard
        Err(_) => return Err("Failed to lock serial_task.".to_string()),
    };
    if let Some(handle) = serial_task_guard.take() {
        handle.abort(); // 直接终止任务
        return Ok("Serial task stopped.".to_string());
    }
    Err("No running serial task.".to_string())
}
#[tauri::command]
fn greet(state: tauri::State<StateWrapper>, name: &str) -> String {
    println!("Hello, {}!", name);
    let portlist = tokio_serial::available_ports().expect("Can't get available port");
    let payload = SerialPortList {
        port_vec: portlist.clone().into_iter().map(|p| p.port_name.to_string()).collect(),
    };
    let app_handler = state.0.try_lock().unwrap().app_handler.clone();
    app_handler.emit("serial_change", payload).expect("未能发送串口事件");
    format!("Hello, {}! You've been greeted from Rust!", name)
}


#[tokio::main]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    tauri::Builder::default().setup(|app| {
        let app_wrapper = StateWrapper(Mutex::new(AppWrapper {
            app_handler: app.handle().clone(),
            // port,
            serial_task: Arc::new(Default::default()),
        }));
        app.manage(app_wrapper);
        let app_handle = Arc::new(Mutex::new(app.handle().clone()));

        Ok(())
    })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet,set_serial_cfg,stop_serial_task])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
