// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod serial;
mod sqlite;

use crate::serial::{
    deinit_device, fetch_hall_data, get_hall, get_laser, get_motor_angle, get_port, hall_parse_data,
    init_device, motor_start_d, motor_start_one_circle, motor_start_u, motor_stop, rotate_motor,
    set_motor_calibrated, set_motor_single_angle, set_motor_single_circle_pulse, set_motor_speed, start_work, stop_work,
};
use crate::sqlite::{connect_to_db, gen_xlsx, get_data_by_parent_id};
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time::timeout;
use tokio_serial::{SerialPortBuilderExt, SerialStream};
use tokio_util::bytes::{Bytes, BytesMut};
use tokio_util::codec::{BytesCodec, Framed};

#[derive(Clone, Serialize)]
struct MessagePayload {
    message: String,
    title: String,
    _type: String,
}
pub struct AppWrapper {
    pub app_handler: AppHandle,
    pub step_pulse: Mutex<u32>,
    single_circle_pulse: Mutex<u32>,
    pub hall_serial: Mutex<Option<Framed<SerialStream, BytesCodec>>>,
    pub motor_serial: Mutex<Option<Framed<SerialStream, BytesCodec>>>,
    pub laser_address: Mutex<Option<String>>,
    pub laser_socket: Mutex<Option<UdpSocket>>,
    pub stop_tx: watch::Sender<bool>,
    hall_buffer: Mutex<VecDeque<Payload>>,
    pub motor_tx: mpsc::Sender<f32>,
    pub motor_rx: Mutex<mpsc::Receiver<f32>>,

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
        let socket = UdpSocket::bind("0.0.0.0:43000")
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
    pub async fn recv_with_timeout(
        serial: &mut Framed<SerialStream, BytesCodec>,
        duration: Duration,
    ) -> Result<BytesMut, String> {
        match timeout(duration, serial.next()).await {
            Ok(Some(Ok(bytes))) => Ok(bytes),              // 成功返回数据
            Ok(Some(Err(e))) => Err(format!("串口接收错误: {}", e)),
            Ok(None) => Err("串口已关闭".to_string()),      // Stream 已结束
            Err(_) => Err("接收超时".to_string()),          // 超时
        }
    }
    pub async fn spawn_motor_listener(self: Arc<Self>) {
        let mut stop_rx = self.stop_tx.subscribe();
        let tx = self.motor_tx.clone();

        tokio::spawn(async move {
            loop {
                if *stop_rx.borrow() {
                    println!("Motor listener stopping...");
                    match self.motor_stop_work().await {
                        Ok(str) => {
                            self.app_handler.emit("message", MessagePayload {
                                title: "关闭成功".to_string(),
                                message: str,
                                _type: "success".to_string(),
                            }).unwrap();
                        }
                        Err(e) => {
                            self.app_handler.emit("message", MessagePayload {
                                title: "关闭失败".to_string(),
                                message: e,
                                _type: "error".to_string(),
                            }).unwrap();
                        }
                    }
                    break;
                }
                match self.recv_res(Duration::from_secs(4)).await {
                    Ok(angle) => {
                        // 收到角度，发到 channel
                        if tx.send(angle).await.is_err() {
                            println!("No receiver for motor data");
                        }
                    }
                    Err(_) => {
                        // 超时不处理，继续等
                        self.app_handler.emit("message", MessagePayload {
                            title: "关闭失败".to_string(),
                            message: "电机控制板通信超时！".into(),
                            _type: "error".to_string(),
                        }).unwrap();
                        break;
                    }
                }
            }
        });
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
        if let Some(Ok(_)) = timeout(Duration::from_secs(20), serial.next())
            .await
            .map_err(|_| "电机响应超时，请检查线路连接！".to_string())?
        {
            Ok(())
        } else {
            Err("No response from Motor".into())
        }
    }

    pub async fn rotate_motor_step(&self) -> Result<(), String> {
        self.rotate_motor_pulse(*self.step_pulse.lock().await).await
    }


    async fn recv_res(&self, duration: Duration) -> Result<f32, String> {
        let mut lock = self.motor_serial.lock().await;
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor not initialized".into()),
        };
        match Self::recv_with_timeout(serial, duration).await {
            Ok(bytes) => {
                // println!("{:X}", bytes);
                let data = &bytes[..];
                if data[0..2] == [0xEF, 0xFE] && data[7..] == [0xFF, 0xEE] {
                    if data[3] == 0x09 {
                        self.stop_tx.send(true).unwrap();
                    }
                    let slice = u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
                    Ok(f32::from_bits(slice))
                } else {
                    Err("响应错误".into())
                }
            }
            Err(_e) => Err("电机控制器响应超时！".into()),
        }
    }

    async fn talk_with_motor(&self, command: u8, value: u32, duration: Duration) -> Result<u32, String> {
        let mut lock = self.motor_serial.lock().await;
        let serial = match lock.as_mut() {
            Some(s) => s,
            None => return Err("Motor not initialized".into()),
        };
        let mut pkg: [u8; 9] = [0xEF, 0xFE, command, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xEE];
        let bytes = value.to_le_bytes();
        pkg[3..7].copy_from_slice(&bytes);
        serial
            .send(Bytes::copy_from_slice(&pkg[..]))
            .await
            .map_err(|e| e.to_string())?;
        match Self::recv_with_timeout(serial, duration).await {
            Ok(bytes) => {
                println!("{:X}", bytes);
                let data = &bytes[..];
                if data[0..2] == [0xEF, 0xFE] && data[7..] == [0xFF, 0xEE] {
                    let slice = u32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
                    Ok(slice)
                } else {
                    Err("响应错误".into())
                }
            }
            Err(_e) => Err("电机控制器响应超时！".into()),
        }
    }


    pub async fn set_motor_single_angle(&self, angle: f32) -> Result<String, String> {
        let value = {
            let single = self.single_circle_pulse.lock().await;
            (angle * (*single) as f32 / 360.0_f32).ceil() as u32
        };
        match self.talk_with_motor(0, value, Duration::from_millis(1000)).await {
            Ok(_) => {
                *self.step_pulse.lock().await = value;
                Ok(format!("设置单步脉冲个数为{}", value))
            }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }
    pub async fn set_single_circle_pulse(&self, pulse: u32) -> Result<String, String> {
        match self.talk_with_motor(1, pulse, Duration::from_millis(1000)).await {
            Ok(_) => {
                let mut value = self.single_circle_pulse.lock().await;
                *value = pulse;
                Ok(format!("设置单圈脉冲个数为{}", pulse))
            }
            _ => {
                Err("电机控制器响应错误，请检查线路！".into())
            }
        }
    }
    pub async fn set_motor_speed(&self, speed: f32) -> Result<String, String> {
        println!("speed: {}", speed);
        let tmp = {
            *self.single_circle_pulse.lock().await
        };
        let value: u32 = (tmp as f32 * speed / 60_f32).ceil() as u32;
        println!("value: {}", value);
        match self.talk_with_motor(2, value, Duration::from_millis(1000)).await {
            Ok(_) => { Ok(format!("设置速度为{}RPM成功!", speed)) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }

    pub async fn set_motor_calibrated(&self) -> Result<String, String> {
        match self.talk_with_motor(3, 0, Duration::from_millis(1000)).await {
            Ok(_) => { Ok("设置原点位置成功!".into()) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }

    pub async fn get_motor_angle(&self) -> Result<f32, String> {
        match self.talk_with_motor(4, 0, Duration::from_millis(1000)).await {
            Ok(angle) => { Ok(f32::from_bits(angle)) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }

    pub async fn motor_start_work(&self) -> Result<String, String> {
        match self.talk_with_motor(5, 0, Duration::from_millis(1000)).await {
            Ok(_) => { Ok("开始检测！".into()) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }

    pub async fn motor_stop_work(&self) -> Result<String, String> {
        match self.talk_with_motor(9, 0, Duration::from_secs(1)).await {
            Ok(_) => {
                Ok("停止任务成功！".into())
            }
            Err(e) => {
                Err(e)
            }
        }
    }
    pub async fn motor_start_u(&self) -> Result<(), String> {
        match self.talk_with_motor(6, 0, Duration::from_millis(1000)).await {
            Ok(_) => { Ok(()) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }
    pub async fn motor_start_d(&self) -> Result<(), String> {
        match self.talk_with_motor(7, 0, Duration::from_millis(1000)).await {
            Ok(_) => { Ok(()) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
    }

    pub async fn motor_stop(&self) -> Result<(), String> {
        match self.talk_with_motor(8, 0, Duration::from_millis(1000)).await {
            Ok(_) => { Ok(()) }
            _ => { Err("电机控制器响应错误，请检查线路！".into()) }
        }
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

#[derive(Clone, serde::Serialize)]
struct SerialPortList {
    port_vec: Vec<PortInfo>,
}

#[tokio::main]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub async fn run() {
    // 创建 stop channel
    let (stop_tx, _stop_rx) = watch::channel(false);
    let (tx, rx) = mpsc::channel(32);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {

            // 初始化 AppWrapper
            let app_wrapper = AppWrapper {
                app_handler: app.handle().clone(),
                step_pulse: 40.into(),
                hall_serial: Default::default(),
                motor_serial: Default::default(),
                laser_address: Default::default(),
                laser_socket: Default::default(),
                single_circle_pulse: 15000.into(),
                stop_tx,
                hall_buffer: Mutex::new(VecDeque::with_capacity(BUFFER_SIZE)),
                motor_tx: tx,
                motor_rx: Mutex::new(rx),
            };

            connect_to_db().expect("Failed to connect to DB");

            // 注入到 Tauri state
            app.manage(Arc::new(app_wrapper));

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
            set_motor_single_circle_pulse,
            get_motor_angle,
            set_motor_calibrated
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
