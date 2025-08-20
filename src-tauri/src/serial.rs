use crate::sqlite::{create_project, insert_data};
use crate::{AppWrapper, Payload, PortInfo, SerialPortList, StateWrapper};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::{watch, Mutex};
use tokio::time::sleep;
use tokio_serial::SerialPortType;
#[derive(Clone, serde::Serialize)]
pub struct LaserData {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub fn hall_parse_data(received: &[u8]) -> Option<Vec<i32>> {
    if received.len() < 44 {
        println!("{:?}", received);
        return None;
    }

    let mut result = Vec::new();

    for i in 0..9 {
        let value = i32::from_le_bytes([
            received[4 + i * 4],
            received[5 + i * 4],
            received[6 + i * 4],
            received[7 + i * 4],
        ]);
        result.push(value);
    }

    Some(result)
}

pub fn laser_parse_data(frames: BTreeMap<u8, Vec<u8>>, angle: f32, laser_d: f32) -> Option<Vec<LaserData>> {
    let mut result = Vec::new();

    // 将角度转换为弧度
    let theta = angle.to_radians();
    let mut count = 0;
    for (_frame_id, data) in frames.iter() {
        if data.len() < 4 {
            // 跳过长度不足的 frame
            continue;
        }

        // 去掉前四个无用字节
        let payload = &data[4..];

        // 每 8 个字节为一组数据包
        let chunk_count = payload.len() / 8;
        for i in 0..chunk_count {
            let chunk = &payload[i * 8..i * 8 + 8];

            // 前四个字节为半径
            let r_bytes = &chunk[0..4];
            let z_bytes = &chunk[4..8];
            let r_tmp = f32::from_le_bytes(r_bytes.try_into().ok()?);
            let z_tmp = f32::from_le_bytes(z_bytes.try_into().ok()?);
            if r_tmp < -70_f32 {
                continue;
            }
            if count == 10 {
                count = 0;
            } else {
                count += 1;
                continue;
            }
            println!("r={},z={}", r_tmp, z_tmp);
            let r = laser_d - 200.0_f32 - r_tmp;
            let z = z_tmp;

            // 极坐标转笛卡尔
            let x = r * theta.cos();
            let y = r * theta.sin();

            result.push(LaserData { x, y, z });
        }
    }

    Some(result)
}

#[tauri::command]
pub async fn get_port(state: tauri::State<'_, StateWrapper>) -> Result<SerialPortList, String> {
    let portlist = tokio_serial::available_ports();
    match portlist {
        Ok(ports) => {
            let payload = SerialPortList {
                port_vec: ports
                    .into_iter()
                    .map(|p| PortInfo {
                        port: p.port_name.clone(),
                        info: match p.port_type {
                            SerialPortType::UsbPort(info) => {
                                info.product.unwrap_or("Unknown Product".to_string())
                            }
                            SerialPortType::PciPort => "PCI Port".to_string(),
                            SerialPortType::BluetoothPort => "Bluetooth Port".to_string(),
                            SerialPortType::Unknown => "串行设备".to_string(),
                        },
                    })
                    .collect(),
            };
            Ok(payload)
        }
        Err(e) => Err(format!("Failed to get serial port: {}", e)),
    }
}
#[tauri::command]
pub async fn init_device(
    state: tauri::State<'_, StateWrapper>,
    hall_port: String,
    motor_port: String,
    laser_addr: String,
) -> Result<String, String> {
    state
        .app
        .try_lock()
        .unwrap()
        .init(&hall_port, &motor_port, laser_addr)
        .await
}

#[tauri::command]
pub async fn deinit_device(state: tauri::State<'_, StateWrapper>) -> Result<String, String> {
    state.app.try_lock().unwrap().deinit().await
}

#[tauri::command]
pub async fn get_hall(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.get_hall_data().await?;
    Ok(())
}

#[tauri::command]
pub async fn rotate_motor(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.rotate_motor_step().await?;
    Ok(())
}

#[tauri::command]
pub async fn get_laser(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.get_laser_data().await?;
    Ok(())
}

#[tauri::command]
pub async fn set_motor_speed(
    state: tauri::State<'_, StateWrapper>,
    speed: f32,
) -> Result<String, String> {
    let guard = state.app.lock().await;
    guard.set_motor_speed(speed).await
}

#[tauri::command]
pub async fn set_motor_single_angle(
    state: tauri::State<'_, StateWrapper>,
    angle: f32,
) -> Result<String, String> {
    let mut guard = state.app.lock().await;
    guard.set_motor_single_angle(angle).await
}

#[tauri::command]
pub async fn motor_start_u(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.motor_start_u().await
}

#[tauri::command]
pub async fn motor_start_one_circle(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.rotate_motor_pulse(guard.single_circle_pulse).await
}

#[tauri::command]
pub async fn motor_start_d(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.motor_start_d().await
}

#[tauri::command]
pub async fn motor_stop(state: tauri::State<'_, StateWrapper>) -> Result<(), String> {
    let guard = state.app.lock().await;
    guard.motor_stop().await
}
#[tauri::command]
pub fn stop_work(state: tauri::State<'_, StateWrapper>) -> Result<String, String> {
    state.stop_tx.send(true).map_err(|e| e.to_string())?;
    Ok("Background task stopping...".into())
}

#[tauri::command]
pub async fn set_motor_single_circle_pulse(state: tauri::State<'_, StateWrapper>, pulse: u32) -> Result<String, String> {
    let mut guard = state.app.lock().await;
    guard.single_circle_pulse = pulse;
    Ok("设置单圈总脉冲成功！".into())
}

#[tauri::command]
pub async fn start_work(
    state: tauri::State<'_, StateWrapper>,
    name: String,
    laser_path: String,
    hall_path: String,
    v_path: String,
    hall_d: f32,
    laser_d: f32,
) -> Result<String, String> {
    let app = state.app.clone(); // Arc<Mutex<AppWrapper>>
    let _ = state.stop_tx.send(false);
    // 创建一个停止信号 channel
    let stop_rx = state.stop_tx.subscribe();
    let shared = state.shared_state.clone();

    // 启动后台任务
    tokio::spawn(async move {
        let laser_path = laser_path.clone();
        let hall_path = hall_path.clone();
        let name = name.clone();
        let hall_d = hall_d.clone();
        let laser_d = laser_d.clone();
        let mut current_pulse: u32 = 0;
        let single_circle_pulse = app.lock().await.single_circle_pulse;
        let parent_id = match create_project(name, hall_d, laser_d) {
            Ok(id) => id,
            Err(e) => {
                let app_lock = app.lock().await;
                app_lock
                    .app_handler
                    .emit("error", json!({ "mes": e }))
                    .expect("TODO: panic message");
                return;
            }
        };
        let step_pulse: u32;
        {
            step_pulse = app.lock().await.step_pulse;
        }
        loop {
            // 检查是否收到停止信号
            if *stop_rx.borrow() {
                println!("Background task stopping...");
                return;
            }
            let current_angle = current_pulse as f32 * 360_f32 / single_circle_pulse as f32;
            // 1. 采集霍尔数据
            {
                let app_lock = app.lock().await;
                match app_lock.get_hall_data().await {
                    Ok(hall_data) => match insert_data(parent_id, current_angle, &hall_data) {
                        Ok(_) => {
                            // app_lock
                            //     .app_handler
                            //     .emit("hall_recv", Payload { angle: current_angle, data: hall_data })
                            //     .expect("TODO: panic message");
                            let mut v_array: Vec<f32> = Vec::new();
                            for hall_datum in &hall_data {
                                let v = (1.65_f32 * (*hall_datum as f32) / 8388607_f32) / 128_f32;
                                v_array.push(v);
                            }
                            let mut v_file = OpenOptions::new()
                                .create(true) // 文件不存在就创建
                                .append(true) // 追加而不是覆盖
                                .open(v_path.clone())
                                .await
                                .expect("无法打开文件");
                            let v_line = format!("{} {} {} {} {} {} {} {} {} {} {}\n",
                                                 current_angle,
                                                 v_array[0],
                                                 v_array[1],
                                                 v_array[2],
                                                 v_array[3],
                                                 v_array[4],
                                                 v_array[5],
                                                 v_array[6],
                                                 v_array[7],
                                                 v_array[8],
                                                 v_array[9],
                            );
                            v_file.write_all(v_line.as_bytes()).await.expect("写入失败");
                            let mut file = OpenOptions::new()
                                .create(true) // 文件不存在就创建
                                .append(true) // 追加而不是覆盖
                                .open(hall_path.clone())
                                .await
                                .expect("无法打开文件");
                            let line = format!(" {} {} {} {} {} {} {} {} {} {} {}\n",
                                               current_angle,
                                               hall_data[0],
                                               hall_data[1],
                                               hall_data[2],
                                               hall_data[3],
                                               hall_data[4],
                                               hall_data[5],
                                               hall_data[6],
                                               hall_data[7],
                                               hall_data[8],
                                               hall_data[9],
                            );
                            file.write_all(line.as_bytes()).await.expect("写入失败");

                            let shared_lock = shared.lock().await;
                            shared_lock.push_hall_data(Payload { angle: current_angle, data: hall_data }).await;
                        }
                        Err(e) => {
                            app_lock
                                .app_handler
                                .emit("error", json!({ "mes": e }))
                                .expect("TODO: panic message");
                        }
                    },
                    Err(e) => {
                        eprintln!("Error getting hall data: {}", e);
                        app_lock
                            .app_handler
                            .emit("error", json!({ "mes": "霍尔传感器响应超时，任务终止！" }))
                            .expect("TODO: panic message");

                        return;
                    }
                }
            }

            // 2. 采集激光位移传感器数据
            {
                let app_lock = app.lock().await;
                match app_lock.get_laser_data().await {
                    Ok(laser_data) => {
                        if let Some(data) = laser_parse_data(laser_data, current_angle, laser_d) {
                            let mut file = OpenOptions::new()
                                .create(true) // 文件不存在就创建
                                .append(true) // 追加而不是覆盖
                                .open(laser_path.clone())
                                .await
                                .expect("无法打开文件");
                            for datum in data {
                                let line = format!("{} {} {}\n", datum.x, datum.y, datum.z);
                                file.write_all(line.as_bytes()).await.expect("写入失败");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error getting laser data: {}", e);
                        return;
                    }
                }
            }

            // 3. 控制电机旋转到当前角度
            {
                let app_lock = app.lock().await;
                if let Err(e) = app_lock.rotate_motor_step().await {
                    eprintln!("Error rotating motor: {}", e);
                    return;
                } else {
                    current_pulse += step_pulse;
                    println!("current angle: {}", current_angle);
                    if current_pulse >= single_circle_pulse {
                        println!("One full rotation completed");
                        break;
                    }
                }
            }

            // 避免 CPU 空转
            sleep(Duration::from_millis(2)).await;
        }
    });

    Ok("Background work started".into())
}

#[tauri::command]
pub async fn fetch_hall_data(state: tauri::State<'_, StateWrapper>) -> Result<Vec<Payload>, String> {
    Ok(state.shared_state.lock().await.fetch_hall_data(1000).await)
}
