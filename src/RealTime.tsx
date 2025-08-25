import {useState, useEffect, useRef} from "react";
import {
    Button, Card,
    Dialog,
    Drawer,
    Form,
    Input,
    InputNumber,
    NotificationPlugin,
    Select,
    Space
} from "tdesign-react";
import {invoke, InvokeArgs} from "@tauri-apps/api/core";
import * as echarts from "echarts";
import {listen} from "@tauri-apps/api/event";
import {save} from '@tauri-apps/plugin-dialog';
import {FileIcon} from 'tdesign-icons-react';

interface port_info {
    port: string;
    info: string;
}

interface hall_data {
    angle: number;
    data: number[];
}

interface serial_list {
    port_vec: port_info[];
}


interface MessagePayload {
    _type: 'info' | 'success' | 'warning' | 'error';
    title: string;
    message: string;
}


async function runInvoke<T>(
    cmd: string,
    args?: InvokeArgs,
    successTitle?: string | null,
    errorTitle?: string | null
): Promise<T> {
    try {
        console.log(args)
        const res_1 = await invoke<T>(cmd, args);
        // 默认成功处理
        await NotificationPlugin.success({
            title: successTitle ? successTitle : "发送成功",
            content: (typeof res_1 == 'string') ? res_1 : '执行成功',
            placement: "top-right",
            duration: 3000,
            offset: [0, 0],
            closeBtn: true,
        });

        return res_1;
    } catch (err_1) {
        // 默认错误处理
        NotificationPlugin.error({
            title: errorTitle ? errorTitle : "请求失败",
            content: String(err_1),
            placement: "top-right",
            duration: 3000,
            offset: [0, 0],
            closeBtn: true,
        });


        // 让错误继续向外抛，以便外层还可以 .catch()
        throw err_1;
    }
}


function RealTime() {
    const [portList, setPortList] = useState<port_info[]>([]);
    const [drawerVisible, setDrawerVisible] = useState(false);
    const [isConnected, setIsConnected] = useState(false);
    const [workDialog, setWorkDialog] = useState<boolean>(false);
    const dataChart = useRef<echarts.ECharts | null>(null);
    const timerID = useRef<number | null>(null);
    const [form] = Form.useForm();
    const [angle, setAngle] = useState<number>(0.00);
    // const bandRateList: number[] = [
    //     110, 300, 600, 1200, 2400, 4800, 9600, 14400, 19200, 38400,
    //     56000, 57600, 115200, 128000, 230400, 256000,
    // ];

    const initChart = () => {
        const chartDom = document.getElementById("main");
        if (!chartDom) return;

        const myChart = echarts.init(chartDom);
        myChart.setOption({
            title: {
                text: "磁场强度",
                textAlign: "center",
                left: "center",
            },
            tooltip: {
                trigger: 'axis',  // 根据 x 轴显示 tooltip，而不是点
            },
            xAxis: [{
                type: "value",
                min: 0,
                max: 360,
                name: "角度(°)",
                interval: 60,
                axisLine: {
                    onZero: false  // 不再对齐 Y 轴零点
                },
            }],
            yAxis: [
                {
                    type: 'value',
                    name: 'ADC值',
                },
                {
                    type: 'value',
                    name: '电压 (V)',
                    axisLabel: {
                        formatter: function (value: number) {
                            return (value * 3.3 / 4095).toFixed(2); // 假设ADC 12位
                        }
                    }
                }
            ],
            legend: {orient: "vertical", right: 30, top: 20, bottom: 20},

            series: Array.from({length: 9}).map((_, index) => ({
                name: `${index + 1}号传感器`,
                type: "line",
                symbolSize: 0,
                yAxisIndex: 0,
                smooth: true,
                data: [],
            })),
            dataZoom: [
                {
                    id: 'dataZoomX',
                    type: 'slider',
                    xAxisIndex: [0],
                    filterMode: 'filter'
                }
            ]
        });
        dataChart.current = myChart;
    };

    // const updateChart = () => {
    //     if (dataChart.current) {
    //         dataChart.current.setOption({
    //             series: Array.from({length: 9}).map((_, index) => ({
    //                 name: `${index + 1}号传感器`,
    //                 type: "line",
    //                 data: dataList.current.map((d) => [d.angle, d.data[index]]),
    //             })),
    //         });
    //     }
    // };

    const updatePort = (portVec: port_info[]) => {
        setPortList(portVec);
    };

    function useHallData(onData: (data: hall_data[]) => void) {
        useEffect(() => {
            // 每隔 200ms 执行一次
            const timer = setInterval(async () => {
                try {
                    // 调用后端命令 fetch_hall_data
                    const data = await invoke<hall_data[]>("fetch_hall_data");
                    // 如果有数据就传给回调
                    if (data.length > 0) {
                        onData(data);
                    }
                } catch (e) {
                    console.error("fetch error", e);
                }
            }, 1000);

            // useEffect 的清理函数，组件卸载时清除定时器
            return () => clearInterval(timer);
        }, [onData]); // 依赖 onData 变化时重新执行
    }

    useEffect(() => {
        let unlisten: (() => void) | undefined;
        // 监听数据
        listen<hall_data>("hall_recv", (event) => {
            const newData = event.payload;
            if (dataChart.current) {
                newData.data.forEach((val, index) => {
                    dataChart.current?.appendData({
                        seriesIndex: index,
                        data: [[newData.angle, val]],
                    });
                });
            }
            dataChart.current?.resize();
        });
        listen<MessagePayload>("message", async (event) => {
            if (event.payload._type == 'error') {
                await NotificationPlugin.error({
                    title: String(event.payload.title),
                    content: String(event.payload.message),
                    placement: "top-right",
                    duration: 3000,
                    offset: [0, 0],
                    closeBtn: true,
                })
            } else if (event.payload._type == 'success') {
                await NotificationPlugin.success({
                    title: String(event.payload.title),
                    content: String(event.payload.message),
                    placement: "top-right",
                    duration: 3000,
                    offset: [0, 0],
                    closeBtn: true,
                })
            }

        }).then((fn) => {
            unlisten = fn
        })

        // 定时刷新图表
        // timerID.current = window.setInterval(updateChart, 500);

        // 初始化图表
        initChart();

        // 监听窗口大小变化
        const resizeHandler = () => {
            dataChart.current?.resize();
        };
        window.addEventListener("resize", resizeHandler);

        return () => {
            if (timerID.current) clearInterval(timerID.current);
            if (unlisten) unlisten(); // 组件卸载时移除监听
            window.removeEventListener("resize", resizeHandler);
        };
    }, []);
    useHallData((dataBatch) => {
        if (dataChart.current) {
            dataBatch.forEach((val) => {
                val.data.forEach((d, i) => {
                    dataChart.current?.appendData({
                        seriesIndex: i,
                        data: [[val.angle, d]],
                    });
                })
            });
        }
        dataChart.current?.resize();
    });
    return (
        <div style={{height: "100vh"}}>
            <div
                id="main"
                style={{
                    width: "100%",
                    height: "90%",
                    backgroundColor: "#ffffff",
                    borderRadius: "16px",
                }}
            ></div>
            <Space direction={'horizontal'} size={'medium'}>
                <Button onClick={() => {
                    setWorkDialog(true)
                }}>
                    开始采集
                </Button>
                <Button theme={'danger'} onClick={async () => {
                    await runInvoke("stop_work")
                }}>
                    停止采集
                </Button>


                <Button
                    shape="rectangle"
                    size="medium"
                    type="button"
                    variant="base"
                    onClick={async () => {
                        setDrawerVisible(true);
                        try {
                            const response = await invoke<serial_list>("get_port", {});
                            updatePort(response.port_vec);
                        } catch (err) {
                            await NotificationPlugin.error({
                                title: "获取串口列表失败",
                                content: String(err),
                                placement: "top-right",
                                duration: 3000,
                                offset: [0, 0],
                                closeBtn: true,
                            });
                        }
                    }}
                >
                    设置
                </Button>

            </Space>


            <Drawer
                header="设置"
                visible={drawerVisible}
                onClose={() => setDrawerVisible(false)}
                footer={
                    <div style={{textAlign: "right"}}>
                        <Button theme={"default"} onClick={() => setDrawerVisible(false)}>
                            取消
                        </Button>
                        <Button type="submit" theme={isConnected ? "danger" : "success"} form="serialForm">
                            {isConnected ? "断开" : "连接"}
                        </Button>
                    </div>
                }
            >
                <Form
                    id="serialForm"
                    labelWidth={120}
                    labelAlign={'left'}
                    initialData={{laserAddr: "192.168.2.3:43002"}}
                    disabled={isConnected}
                    onSubmit={(m) => {
                        if (isConnected) {
                            runInvoke("deinit_device", {}, "成功").then(() => {
                                setIsConnected(false);
                            })
                        } else {
                            runInvoke("init_device", m.fields, "成功").then(() => {
                                setIsConnected(true);
                            })
                        }
                    }}
                >
                    <Form.FormItem label="霍尔端口号" name={"hallPort"}>
                        <Select>
                            {portList.map((port) => (
                                <Select.Option key={port.port} style={{height: "60px"}} value={port.port}
                                               label={port.port}>
                                    <div style={{marginLeft: "16px"}}>
                                        <div>{port.port}</div>
                                        <div style={{
                                            fontSize: "13px",
                                            color: "var(--td-gray-color-9)"
                                        }}>{port.info}</div>
                                    </div>
                                </Select.Option>
                            ))}
                        </Select>
                    </Form.FormItem>
                    <Form.FormItem label="电机端口号" name={"motorPort"}>
                        <Select>
                            {portList.map((port) => (
                                <Select.Option key={port.port} style={{height: "60px"}} value={port.port}
                                               label={port.port}>
                                    <div style={{marginLeft: "16px"}}>
                                        <div>{port.port}</div>
                                        <div style={{
                                            fontSize: "13px",
                                            color: "var(--td-gray-color-9)"
                                        }}>{port.info}</div>
                                    </div>
                                </Select.Option>
                            ))}
                        </Select>
                    </Form.FormItem>
                    <Form.FormItem label="激光传感器地址" name={"laserAddr"}>
                        <Input></Input>
                    </Form.FormItem>
                </Form>
                <Space direction={'vertical'} size={'medium'} style={{width: '100%', marginTop: '10px'}}>
                    <Form
                        id={"MotorSpeed"}
                        labelWidth={60}
                        disabled={!isConnected}
                        labelAlign={'left'}
                        initialData={{speed: 0.5}}
                        onSubmit={(m) => {
                            runInvoke<string>('set_motor_speed', m.fields)
                        }}>
                        <Space size={'small'}>
                            <Form.FormItem name={'speed'} label={'电机转速'}>
                                <InputNumber theme="normal" suffix={'RPM'}></InputNumber>
                            </Form.FormItem>
                            <Button disabled={!isConnected} type={'submit'}
                                    form={'MotorSpeed'}>变更</Button>
                        </Space>
                    </Form>
                    <Form
                        id={"MotorSingleAngle"}
                        labelWidth={60}
                        disabled={!isConnected}
                        labelAlign={'left'}
                        initialData={{angle: 1}}
                        onSubmit={(m) => {
                            runInvoke<string>('set_motor_single_angle', m.fields)
                        }}>
                        <Space size={'small'}>
                            <Form.FormItem name={'angle'} label={'单步间距'}>
                                <InputNumber theme="normal" suffix={'°/step'}></InputNumber>
                            </Form.FormItem>
                            <Button disabled={!isConnected} type={'submit'}
                                    form={'MotorSingleAngle'}>变更</Button>
                        </Space>
                    </Form>
                    <Form
                        id={"MotorPulse"}
                        labelWidth={60}
                        disabled={!isConnected}
                        labelAlign={'left'}
                        initialData={{pulse: 15000}}
                        onSubmit={(m) => {
                            runInvoke<string>('set_motor_single_circle_pulse', m.fields)
                        }}>
                        <Space size={'small'}>
                            <Form.FormItem name={'pulse'} label={'一圈脉冲'}>
                                <InputNumber theme="normal" suffix={'step'}></InputNumber>
                            </Form.FormItem>
                            <Button disabled={!isConnected} type={'submit'}
                                    form={'MotorPulse'}>变更</Button>
                        </Space>
                    </Form>
                    <Card>当前角度：{angle}°</Card>
                    <Button disabled={!isConnected} block onClick={async () => {
                        await runInvoke<number>("get_motor_angle").then((res) => {
                            setAngle(res)
                        })
                    }}>
                        检测角度
                    </Button>
                    <Button disabled={!isConnected} block onClick={async () => {
                        await runInvoke<string>("set_motor_calibrated").then((_) => {
                            setAngle(0.00)
                        })
                    }}>
                        角度清零
                    </Button>
                    <Button disabled={!isConnected} block onClick={() => {
                        invoke("motor_start_u")
                    }}>手动正转</Button>
                    <Button disabled={!isConnected} block onClick={() => {
                        invoke("motor_start_d")
                    }}>手动反转</Button>
                    <Button disabled={!isConnected} block theme={'danger'} onClick={() => {
                        invoke("motor_stop")
                    }}>停止</Button>

                </Space>

            </Drawer>

            <Dialog
                header="任务信息"
                visible={workDialog}
                onClose={() => {
                    setWorkDialog(false)
                }}
                footer={

                    <>
                        <Button theme={'default'} onClick={() => {
                            setWorkDialog(false)
                        }}>
                            取消
                        </Button>
                        <Button theme="primary" type={'submit'} form={'workInfo'} onClick={() => {
                            setWorkDialog(false)
                        }}>
                            开始
                        </Button>
                    </>

                }

            >
                <Form id={'workInfo'} form={form} labelWidth={120} onSubmit={async (m) => {
                    console.log(m.fields)
                    if (dataChart.current) {
                        dataChart.current.setOption({
                            series: Array.from({length: 9}).map((_, index) => ({
                                name: `${index + 1}号传感器`,
                                type: "line",
                                symbolSize: 0,
                                yAxisIndex: 0,
                                smooth: true,
                                data: [],  // 清空数据
                            }))
                        }, false); // 第二个参数 false 表示不要合并
                    }
                    await runInvoke("start_work", m.fields, "开始采集")
                }}
                      initialData={{hallD: 20, laserD: 428}}
                >
                    <Space direction={'vertical'}>
                        <Form.FormItem name={'name'} label={'采集备注'}
                                       rules={[{required: true, message: '请输入备注信息'}]}>
                            <Input></Input>
                        </Form.FormItem>
                        <Form.FormItem name={'hallD'} label={'霍尔距离'}
                                       rules={[{required: true, message: '请输入霍尔距离'}]}>
                            <InputNumber style={{width: '100%'}} suffix={'mm'}></InputNumber>
                        </Form.FormItem>

                        <Form.FormItem name={'laserD'} label={'激光距离'}
                                       rules={[{required: true, message: '请输入激光距离'}]}>
                            <InputNumber style={{width: '100%'}} suffix={'mm'} autoWidth></InputNumber>
                        </Form.FormItem>
                        <Space direction={'horizontal'}>
                            <Form.FormItem name={'laserPath'} label={'外形存储路径'}
                                           rules={[{required: true, message: '请选择存储路径'}]}>
                                <Input></Input>
                            </Form.FormItem>
                            <Button onClick={async () => {
                                const selectPath = await save({
                                    filters: [
                                        {
                                            extensions: ['pts'],
                                            name: ""
                                        },
                                    ],
                                });
                                if (selectPath) {
                                    console.log(selectPath)
                                    form.setFieldsValue({laserPath: selectPath})
                                }
                            }}>
                                <FileIcon></FileIcon>
                            </Button>
                        </Space>
                        <Space direction={'horizontal'}>
                            <Form.FormItem name={'hallPath'} label={'磁场存储路径'}
                                           rules={[{required: true, message: '请选择存储路径'}]}>
                                <Input></Input>
                            </Form.FormItem>
                            <Button onClick={async () => {
                                const selectPath = await save({
                                    filters: [
                                        {
                                            extensions: ['txt'],
                                            name: ""
                                        },
                                    ],
                                });
                                if (selectPath) {
                                    console.log(selectPath)
                                    form.setFieldsValue({hallPath: selectPath})
                                }
                            }}>
                                <FileIcon></FileIcon>
                            </Button>
                        </Space>
                        <Space direction={'horizontal'}>
                            <Form.FormItem name={'vPath'} label={'电压存储路径'}
                                           rules={[{required: true, message: '请选择存储路径'}]}>
                                <Input></Input>
                            </Form.FormItem>
                            <Button onClick={async () => {
                                const selectPath = await save({
                                    filters: [
                                        {
                                            extensions: ['txt'],
                                            name: ""
                                        },
                                    ],
                                });
                                if (selectPath) {
                                    console.log(selectPath)
                                    form.setFieldsValue({vPath: selectPath})
                                }
                            }}>
                                <FileIcon></FileIcon>
                            </Button>
                        </Space>
                    </Space>


                </Form>
            </Dialog>


        </div>
    );
}

export default RealTime;