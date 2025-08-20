// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod serial;
mod sqlite;

use std::char::from_u32;
use crate::serial::{
    deinit_device, fetch_hall_data, get_hall, get_laser, get_port, hall_parse_data, init_device,
    motor_start_d, motor_start_u, motor_stop, rotate_motor, set_motor_single_angle, set_motor_speed,
    start_work, stop_work, motor_start_one_circle, set_motor_single_circle_pulse,
};
use crate::sqlite::{connect_to_db, gen_xlsx, get_data_by_parent_id};
use futures::{SinkExt, StreamExt, TryStreamExt};
use std::collections::{BTreeMap, VecDeque};
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::net::UdpSocket;
use tokio::sync::{watch, Mutex};
use tokio::time::timeout;
use tokio_serial::{SerialPort, SerialPortBuilderExt, SerialStream};
use tokio_util::bytes::Bytes;
use tokio_util::codec::{BytesCodec, Framed};
pub struct AppWrapper {
    pub app_handler: AppHandle,
    pub step_pulse: u32,
    pub single_circle_pulse: u32,
    pub hall_serial: Arc<Mutex<Option<Framed<SerialStream, BytesCodec>>>>,
    pub motor_serial: Arc<Mutex<Option<Framed<SerialStream, BytesCodec>>>>,
    pub laser_address: Arc<Mutex<Option<String>>>,
    pub laser_socket: Arc<Mutex<Option<UdpSocket>>>,
}
impl AppWrapper {
    /// 初始化霍尔串口和电机串口
    pub async fn init(
        &self,
        hall_port: &str,
        motor_port: &str,
        laser_addr: String,
    ) -> Result<String, String> {
        // 初始化霍尔串口
        let hall = tokio_serial::new(hall_port, 115200)
            .open_native_async()
            .map_err(|e| e.to_string())?;
        let hall_framed = Framed::new(hall, BytesCodec::new());
        *self.hall_serial.lock().await = Some(hall_framed);

        // 初始化电机串口
        let motor = tokio_serial::new(motor_port, 115200)
            .open_native_async()
            .map_err(|e| e.to_string())?;
        let motor_framed = Framed::new(motor, BytesCodec::new());
        *self.motor_serial.lock().await = Some(motor_framed);
        *self.laser_address.lock().await = Some(laser_addr.clone());
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:43000")
            .await
            .map_err(|e| e.to_string())?;
        socket
            .connect(laser_addr)
            .await
            .map_err(|e| e.to_string())?;
        *self.laser_socket.lock().await = Some(socket);
        Ok("连接成功!".to_string())
    }
    pub async fn deinit(&self) -> Result<String, String> {
        // 释放霍尔串口
        {
            let mut hall_lock = self.hall_serial.lock().await;
            if hall_lock.is_some() {
                *hall_lock = None; // Framed<T> 实现了 Drop，会自动关闭串口
                println!("Hall serial deinitialized");
            }
        }

        // 释放电机串口
        {
            let mut motor_lock = self.motor_serial.lock().await;
            if motor_lock.is_some() {
                *motor_lock = None;
                println!("Motor serial deinitialized");
            }
        }

        // 释放激光 UDP socket
        {
            let mut socket_lock = self.laser_socket.lock().await;
            if socket_lock.is_some() {
                *socket_lock = None;
                println!("Laser socket deinitialized");
            }
        }

        // 清空激光地址
        {
            let mut addr_lock = self.laser_address.lock().await;
            *addr_lock = None;
        }

        Ok("断开成功!".to_string())
    }

    pub fn set_single_circle_pulse(&mut self, pulse: u32) {
        self.single_circle_pulse = pulse;
        println!("Set single circle pulse{}", self.single_circle_pulse);
    }

    pub async fn get_hall_data(&self) -> Result<Vec<i32>, String> {
        let mut lock = self.hall_serial.lock().await;

        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Hall serial not initialized".into()),
        };
        let pkg: [u8; 5] = [0xFF, 0xEE, 0xAA, 0xEF, 0xFE];

        // 发送命令
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;

        let mut buf = Vec::new();
        let expected_len = 44;

        while buf.len() < expected_len {
            if let Some(Ok(bytes)) = timeout(Duration::from_secs(2), serial.next()).await.map_err(|e| e.to_string())? {
                buf.extend_from_slice(&bytes);
            } else {
                return Err("Timeout waiting for hall data".into());
            }
        }

        // 现在 buf 一定是 >= 44，可以截取前 44
        let buf = buf[..expected_len].to_vec();
        match hall_parse_data(&buf) {
            Some(res) => Ok(res),
            None => Err("Hall serial parse error".into()),
        }
    }

    pub async fn rotate_motor_pulse(&self, pulse: u32) -> Result<(), String> {
        let mut lock = self.motor_serial.lock().await;
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor serial not initialized".into()),
        };
        let mut pkg: [u8; 9] = [0xEF, 0xFE, 0x01, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        let bytes = pulse.to_le_bytes();
        pkg[3..7].copy_from_slice(&bytes);
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;

        // 等待返回
        if let Some(Ok(bytes)) = timeout(Duration::from_secs(20), serial.next())
            .await
            .map_err(|_| "电机响应超时，请检查线路连接！".to_string())?
        {
            Ok(())
        } else {
            Err("No response from Motor".into())
        }
    }


    pub async fn rotate_motor_step(&self) -> Result<(), String> {
        self.rotate_motor_pulse(self.step_pulse).await
    }

    pub async fn set_motor_speed(&self, speed: f32) -> Result<String, String> {
        let mut lock = self.motor_serial.lock().await;

        // 解包 Option
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor serial not initialized".into()),
        };
        let mut pkg: [u8; 9] = [0xEF, 0xFE, 0x05, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        let bytes = speed.to_le_bytes();
        pkg[3..7].copy_from_slice(&bytes);
        // 发送命令
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;

        // 等待返回
        if let Some(Ok(bytes)) = timeout(Duration::from_secs(3), serial.next())
            .await
            .map_err(|_| "电机响应超时，请检查线路连接！".to_string())?
        {
            let data = &bytes[..]; // 转成切片方便匹配

            if data.len() >= 5 && data[0..5] == [0xEF, 0xFE, 0x06, 0xFF, 0xEE] {
                println!("Got expected response from motor!");
                // 这里可以处理正确返回
                Ok("速度设置成功".to_string())
            } else {
                println!("Unexpected response: {:X?}", data);
                Err("Invalid response from motor".into())
            }
        } else {
            Err("No response from Motor".into())
        }
    }

    pub async fn set_motor_single_angle(&mut self, angle: f32) -> Result<String, String> {
        let pulse = angle * (self.single_circle_pulse) as f32 / 360.0_f32;
        self.step_pulse = pulse as u32;
        Ok(format!("设置单步脉冲个数为{}", pulse))
    }
    pub async fn motor_start_u(&self) -> Result<(), String> {
        let mut lock = self.motor_serial.lock().await;

        // 解包 Option
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor serial not initialized".into()),
        };
        let pkg: [u8; 9] = [0xEF, 0xFE, 0x02, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        // 发送命令
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub async fn motor_start_d(&self) -> Result<(), String> {
        let mut lock = self.motor_serial.lock().await;

        // 解包 Option
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor serial not initialized".into()),
        };
        let pkg: [u8; 9] = [0xEF, 0xFE, 0x03, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        // 发送命令
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn motor_stop(&self) -> Result<(), String> {
        let mut lock = self.motor_serial.lock().await;

        // 解包 Option
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor serial not initialized".into()),
        };
        let pkg: [u8; 9] = [0xEF, 0xFE, 0x04, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        // 发送命令
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_laser_data(&self) -> Result<BTreeMap<u8, Vec<u8>>, String> {
        let mut lock = self.laser_socket.lock().await;
        let socket = match lock.as_mut() {
            Some(a) => a, // 这里 clone 一个 String
            None => return Err("Laser socket not initialized".into()),
        };
        let mut lese = [0u8; 2048];
        while let Ok(size) = socket.try_recv(&mut lese) {
            if size == 0 {
                break; //清空接收缓冲区
            }
        }
        let get_data_pkg: [u8; 8] = [0xAA, 0x55, 0x55, 0xAA, 0x02, 0x00, 0x21, 0x01];
        socket
            .send(&get_data_pkg)
            .await
            .map_err(|e| e.to_string())?;
        let mut frames = BTreeMap::new();
        while frames.len() < 8 {
            let mut buf = [0u8; 2048];
            let len = timeout(Duration::from_millis(200), socket.recv(&mut buf))
                .await
                .map_err(|_| "Timeout while receiving frame")?
                .map_err(|e| e.to_string())?;

            if len < 1 {
                return Err("Received empty frame".into());
            }

            let frame_id = buf[len - 1]; // 最后一个字节是帧号
            frames.insert(frame_id, buf[..len - 1].to_vec()); // 去掉帧号保存
            println!("Frame ID: {}", frame_id);
        }
        Ok(frames)
    }
}
#[derive(Clone, serde::Serialize)]
pub struct PortInfo {
    pub port: String,
    pub info: String,
}
#[derive(Clone, serde::Serialize)]
pub struct Payload {
    angle: f32,
    data: Vec<i32>,
}
const BUFFER_SIZE: usize = 10000; // 环形缓冲区大小

#[derive(Clone)]
pub struct SharedState {
    hall_buffer: Arc<Mutex<VecDeque<Payload>>>,
}


// 后端写入数据 (替代 emit)
impl SharedState {
    pub async fn push_hall_data(&self, payload: Payload) {
        let mut buf = self.hall_buffer.lock().await;
        if buf.len() >= BUFFER_SIZE {
            buf.pop_front(); // 丢弃最旧的
        }
        buf.push_back(payload);
    }

    pub async fn fetch_hall_data(&self, max: usize) -> Vec<Payload> {
        let mut buf = self.hall_buffer.lock().await;
        let mut res = Vec::new();
        for _ in 0..max {
            if let Some(item) = buf.pop_front() {
                res.push(item);
            } else {
                break;
            }
        }
        res
    }
}
#[derive(Clone, serde::Serialize)]
struct SerialPortList {
    port_vec: Vec<PortInfo>,
}
pub struct StateWrapper {
    pub app: Arc<Mutex<AppWrapper>>,
    pub stop_tx: watch::Sender<bool>,
    pub shared_state: Arc<Mutex<SharedState>>,
}

#[tokio::main]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    // 创建 stop channel
    let (stop_tx, _stop_rx) = watch::channel(false);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化 AppWrapper
            let app_wrapper = StateWrapper {
                app: Arc::new(Mutex::new(AppWrapper {
                    app_handler: app.handle().clone(),
                    step_pulse: 40,
                    hall_serial: Arc::new(Default::default()),
                    motor_serial: Arc::new(Default::default()),
                    laser_address: Arc::new(Default::default()),
                    laser_socket: Arc::new(Default::default()),
                    single_circle_pulse: 15000,
                })),
                stop_tx, // 放入 stop sender
                shared_state: Arc::new(Mutex::new(SharedState {
                    hall_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_SIZE))),
                })),
            };

            connect_to_db().expect("Failed to connect to DB");

            // 注入到 Tauri state
            app.manage(app_wrapper);

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_data_by_parent_id,
            gen_xlsx,
            get_port,
            init_device,
            get_hall,
            get_laser,
            rotate_motor,
            set_motor_single_angle,
            set_motor_speed,
            deinit_device,
            motor_start_u,
            motor_start_d,
            motor_stop,
            start_work,
            stop_work,
            fetch_hall_data,
            motor_start_one_circle,
            set_motor_single_circle_pulse
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
