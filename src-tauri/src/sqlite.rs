use crate::{AppWrapper};
use chrono::Local;
use rusqlite::{params, Connection};
use tauri::path::BaseDirectory;
use tauri::Manager;
use umya_spreadsheet;

#[derive(Clone, serde::Serialize)]
pub struct Data {
    id: i32,
    parent_id: i32,
    angle: i32,
    data1: i32,
    data2: i32,
    data3: i32,
    data4: i32,
    data5: i32,
    data6: i32,
    data7: i32,
    data8: i32,
    data9: i32,
}

pub fn check_project_table_is_exit() -> Result<Connection, String> {
    let conn = Connection::open("sqlite.db").expect("Can't open sqlite.db");
    match conn.execute(
        "CREATE TABLE IF NOT EXISTS project (\
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    hall_d REAL NOT NULL,
    laser_d REAL NOT NULL,
    time DATETIME)",
        [],
    ) {
        Ok(_) => Ok(conn),
        Err(_) => Err(String::from("Can't create project")),
    }
}

pub fn create_project(name: String, hall_d: f32, laser_d: f32) -> Result<i64, String> {
    let conn = check_project_table_is_exit().expect("Can't create project table");
    match conn.execute(
        "INSERT INTO project (name,time,hall_d,laser_d) VALUES (?,?,?,?)",
        // 将 `data` 中的值绑定到 SQL 语句中的占位符
        params![name, Local::now().timestamp(),hall_d,laser_d],
    ) {
        Ok(_) => {
            println!("Data inserted successfully");
            Ok(conn.last_insert_rowid()) // 插入成功，返回 Ok
        }
        Err(e) => {
            eprintln!("Error inserting data: {}", e);
            Err(e.to_string()) // 如果插入失败，返回错误信息
        }
    }
}

pub fn connect_to_db() -> Result<Connection, String> {
    let conn = Connection::open("sqlite.db").expect("Can't open sqlite.db");
    match conn.execute(
        "CREATE TABLE IF NOT EXISTS data (\
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    parent_id INTEGER NOT NULL,
    angle REAL NOT NULL,
    data1 INTEGER,\
    data2 INTEGER,\
    data3 INTEGER,\
    data4 INTEGER,\
    data5 INTEGER,\
    data6 INTEGER,\
    data7 INTEGER,\
    data8 INTEGER,\
    data9 INTEGER)",
        [],
    ) {
        Ok(_) => Ok(conn),
        Err(_) => Err(String::from("Can't open sqlite.db")),
    }
}

pub fn insert_data(parent_id: i64, angle: f32, data: &Vec<i32>) -> Result<(), String> {
    let conn = connect_to_db().expect("Failed to connect to DB");
    if data.len() != 9 {
        return Err("Data must contain exactly 9 elements".to_string());
    }

    // 使用 `conn.execute` 执行 INSERT INTO 语句
    match conn.execute(
        "INSERT INTO data (parent_id, angle, data1, data2, data3, data4, data5, data6, data7, data8, data9) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        // 将 `data` 中的值绑定到 SQL 语句中的占位符
        params![
            parent_id,  angle,
            data[0], data[1], data[2], data[3], data[4],
            data[5], data[6], data[7], data[8]
        ],
    ) {
        Ok(_) => {
            println!("Data inserted successfully");
            Ok(()) // 插入成功，返回 Ok
        }
        Err(e) => {
            eprintln!("Error inserting data: {}", e);
            Err(e.to_string()) // 如果插入失败，返回错误信息
        }
    }
}

#[tauri::command]
pub fn get_data_by_parent_id(
    _state: tauri::State<AppWrapper>,
    parent_id: i32,
) -> Result<Vec<Data>, String> {
    let conn = connect_to_db().expect("Failed to connect to DB");
    let mut stmt = conn
        .prepare("SELECT * FROM data WHERE WHERE parent_id = ?")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([parent_id], |row| {
            Ok(Data {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                angle: row.get(2)?,
                data1: row.get(3)?,
                data2: row.get(4)?,
                data3: row.get(5)?,
                data4: row.get(6)?,
                data5: row.get(7)?,
                data6: row.get(8)?,
                data7: row.get(9)?,
                data8: row.get(10)?,
                data9: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut data_list = Vec::new();
    for row in rows {
        match row {
            Ok(data) => data_list.push(data),
            Err(e) => eprintln!("Error fetching row: {}", e),
        }
    }

    Ok(data_list)
}
#[tauri::command]
pub fn gen_xlsx(state: tauri::State<AppWrapper>, parent_id: i32) -> Result<String, String> {
    let mut book = umya_spreadsheet::new_file();
    let conn = connect_to_db().expect("Failed to connect to DB");
    let sheet = book.get_sheet_by_name_mut("Sheet1").unwrap();
    sheet.get_cell_mut("A1").set_value("角度");
    sheet.get_cell_mut("B1").set_value("数据1");
    sheet.get_cell_mut("C1").set_value("数据2");
    sheet.get_cell_mut("D1").set_value("数据3");
    sheet.get_cell_mut("E1").set_value("数据4");
    sheet.get_cell_mut("F1").set_value("数据5");
    sheet.get_cell_mut("G1").set_value("数据6");
    sheet.get_cell_mut("H1").set_value("数据7");
    sheet.get_cell_mut("I1").set_value("数据8");
    sheet.get_cell_mut("J1").set_value("数据9");

    let mut stmt = conn.prepare("SELECT angle, data1, data2, data3, data4, data5, data6, data7, data8, data9 FROM data WHERE parent_id = ?")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([parent_id], |row| {
            Ok((
                row.get::<_, i32>(0)?, // 直接获取数据库中的时间字符串
                row.get::<_, i32>(1)?, // 获取数据列
                row.get::<_, i32>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, i32>(4)?,
                row.get::<_, i32>(5)?,
                row.get::<_, i32>(6)?,
                row.get::<_, i32>(7)?,
                row.get::<_, i32>(8)?,
                row.get::<_, i32>(9)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut row_index = 0;
    for (i, row) in rows.enumerate() {
        match row {
            Ok(d) => {
                row_index = i + 2;
                sheet
                    .get_cell_mut(format!("A{}", row_index.to_string()).as_str())
                    .set_value(&d.0.to_string());
                sheet
                    .get_cell_mut(format!("B{}", row_index.to_string()).as_str())
                    .set_value(&d.1.to_string());
                sheet
                    .get_cell_mut(format!("C{}", row_index.to_string()).as_str())
                    .set_value(&d.2.to_string());
                sheet
                    .get_cell_mut(format!("D{}", row_index.to_string()).as_str())
                    .set_value(&d.3.to_string());
                sheet
                    .get_cell_mut(format!("E{}", row_index.to_string()).as_str())
                    .set_value(&d.4.to_string());
                sheet
                    .get_cell_mut(format!("F{}", row_index.to_string()).as_str())
                    .set_value(&d.5.to_string());
                sheet
                    .get_cell_mut(format!("G{}", row_index.to_string()).as_str())
                    .set_value(&d.6.to_string());
                sheet
                    .get_cell_mut(format!("H{}", row_index.to_string()).as_str())
                    .set_value(&d.7.to_string());
                sheet
                    .get_cell_mut(format!("I{}", row_index.to_string()).as_str())
                    .set_value(&d.8.to_string());
                sheet
                    .get_cell_mut(format!("J{}", row_index.to_string()).as_str())
                    .set_value(&d.9.to_string());
            }
            Err(_) => {}
        }
    }
    let app_handle = &state.app_handler;
    let mut path = app_handle
        .path()
        .resolve("", BaseDirectory::AppCache)
        .unwrap();
    path.push("table.xlsx");
    let r = umya_spreadsheet::writer::xlsx::write(&book, path);
    match r {
        Ok(_) => Ok(format!("导出成功，共导出{}条数据!", row_index - 2)),
        Err(e) => Err(format!("导出失败: {}", e)),
    }
}
