use crate::sqlite::{create_project, insert_data};
use crate::{AppWrapper, MessagePayload, Payload, PortInfo, SerialPortList};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Receiver;
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
pub async fn get_port(_: tauri::State<'_, Arc<AppWrapper>>) -> Result<SerialPortList, String> {
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
    app: tauri::State<'_, Arc<AppWrapper>>,
    hall_port: String,
    motor_port: String,
    laser_addr: String,
) -> Result<String, String> {
    app
        .init(&hall_port, &motor_port, laser_addr)
        .await
}

#[tauri::command]
pub async fn deinit_device(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<String, String> {
    app.deinit().await
}

#[tauri::command]
pub async fn get_hall(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.get_hall_data().await?;
    Ok(())
}

#[tauri::command]
pub async fn rotate_motor(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.rotate_motor_step().await?;
    Ok(())
}

#[tauri::command]
pub async fn get_laser(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.get_laser_data().await?;
    Ok(())
}

#[tauri::command]
pub async fn set_motor_speed(
    app: tauri::State<'_, Arc<AppWrapper>>,
    speed: f32,
) -> Result<String, String> {
    app.set_motor_speed(speed).await
}

#[tauri::command]
pub async fn set_motor_single_angle(
    app: tauri::State<'_, Arc<AppWrapper>>,
    angle: f32,
) -> Result<String, String> {
    app.set_motor_single_angle(angle).await
}
#[tauri::command]
pub async fn get_motor_angle(
    app: tauri::State<'_, Arc<AppWrapper>>,
) -> Result<f32, String> {
    app.get_motor_angle().await
}

#[tauri::command]
pub async fn set_motor_calibrated(
    app: tauri::State<'_, Arc<AppWrapper>>,
) -> Result<String, String> {
    app.set_motor_calibrated().await
}
#[tauri::command]
pub async fn motor_start_one_circle(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.rotate_motor_pulse(*app.single_circle_pulse.lock().await).await
}
#[tauri::command]
pub async fn motor_start_u(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.motor_start_u().await
}
#[tauri::command]
pub async fn motor_start_d(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.motor_start_d().await
}

#[tauri::command]
pub async fn motor_stop(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<(), String> {
    app.motor_stop().await
}
#[tauri::command]
pub fn stop_work(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<String, String> {
    app.stop_tx.send(true).map_err(|e| e.to_string())?;
    Ok("Background task stopping...".into())
}

#[tauri::command]
pub async fn set_motor_single_circle_pulse(app: tauri::State<'_, Arc<AppWrapper>>, pulse: u32) -> Result<String, String> {
    app.set_single_circle_pulse(pulse).await
}


#[tauri::command]
pub async fn start_work(
    app: tauri::State<'_, Arc<AppWrapper>>,
    name: String,
    laser_path: String,
    hall_path: String,
    v_path: String,
    hall_d: f32,
    laser_d: f32,
) -> Result<String, String> {
    // Arc<Mutex<AppWrapper>>
    let _ = app.stop_tx.send(false);
    // 创建一个停止信号 channel
    let stop_rx = app.stop_tx.subscribe();
    let app = Arc::clone(&app);
    app.motor_start_work().await?;
    // 启动监听任务（只启动一次即可）
    app.clone().spawn_motor_listener().await;
    let parent_id = match create_project(name.clone(), hall_d, laser_d) {
        Ok(id) => id,
        Err(e) => {
            return Err("数据库异常！".into());
        }
    };
    tokio::spawn(async move {
        let mut lock = app.motor_rx.lock().await;
        let mut stop_rx = stop_rx; // mutable
        let mut laser_file = OpenOptions::new()
            .create(true) // 文件不存在就创建
            .append(true) // 追加而不是覆盖
            .open(laser_path)
            .await
            .expect("无法打开文件");
        let mut hall_file = OpenOptions::new()
            .create(true) // 文件不存在就创建
            .append(true) // 追加而不是覆盖
            .open(hall_path)
            .await
            .expect("无法打开文件");
        let mut v_file = OpenOptions::new()
            .create(true) // 文件不存在就创建
            .append(true) // 追加而不是覆盖
            .open(v_path)
            .await
            .expect("无法打开文件");
        loop {
            tokio::select! {
                angle = lock.recv() => {
                    match angle {
                        Some(a) => {
                            let hall_data = app.get_hall_data().await;
                            let laser_data = app.get_laser_data().await;
                            match hall_data {
                                Ok(data) => {
                                    match insert_data(parent_id, a, &data) {
                                        Ok(_) => {
                                            let mut v_array: Vec<f32> = Vec::new();
                                            for hall_datum in &data {
                                                let v = (1650_f32 * (*hall_datum as f32) / 8388607_f32) / 64_f32;
                                                v_array.push(v);
                                            }
                                            let v_line = format!("{} {} {} {} {} {} {} {} {} {}\n",
                                                                 a,
                                                                 v_array[0],
                                                                 v_array[1],
                                                                 v_array[2],
                                                                 v_array[3],
                                                                 v_array[4],
                                                                 v_array[5],
                                                                 v_array[6],
                                                                 v_array[7],
                                                                 v_array[8],
                                            );
                                            v_file.write_all(v_line.as_bytes()).await.expect("写入失败");
                                            let line = format!(" {} {} {} {} {} {} {} {} {} {}\n",
                                                               a,
                                                               data[0],
                                                               data[1],
                                                               data[2],
                                                               data[3],
                                                               data[4],
                                                               data[5],
                                                               data[6],
                                                               data[7],
                                                               data[8],
                                            );
                                            hall_file.write_all(line.as_bytes()).await.expect("写入失败");
                                            app.push_hall_data(Payload { angle:a, data }).await;
                                        }
                                        Err(e) => {
                                            app.app_handler.emit("message", MessagePayload {
                                                title: "霍尔传感器异常".to_string(),
                                                message: e,
                                                _type: "error".to_string(),
                                            }).unwrap();
                                        }
                                    }
                                }

                                Err(e) => {
                                    eprintln!("Error getting hall data: {}", e);
                                    app.app_handler.emit("message", MessagePayload {
                                        title: "霍尔传感器异常".to_string(),
                                        message: e,
                                        _type: "error".to_string(),
                                    }).unwrap();
                                    return;
                                }
                            }
                            match laser_data {
                                Ok(data) => {
                                    if let Some(data) = laser_parse_data(data, a, laser_d) {
                                        for datum in data {
                                            let line = format!("{} {} {}\n", datum.x, datum.y, datum.z);
                                            laser_file.write_all(line.as_bytes()).await.expect("写入失败");
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error getting laser data: {}", e);
                                    return;
                                }
                            }
                            println!("Angle={}", a);
                            // TODO: 拿传感器数据并存储
                        }
                        None => break, // channel 关闭
                    }
                }

                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        println!("Motor listener stopping...");
                        break;
                    }
                }
            }
        }
    });

    Ok("任务已启动".into())
}

#[tauri::command]
pub async fn fetch_hall_data(app: tauri::State<'_, Arc<AppWrapper>>) -> Result<Vec<Payload>, String> {
    Ok(app.fetch_hall_data(1000).await)
}
