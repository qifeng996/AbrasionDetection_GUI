import {Component} from "react";
// import {invoke} from "@tauri-apps/api/core";
import {Button, Drawer, Form, NotificationPlugin, Select, Tabs} from "tdesign-react";
import './App.css'
import 'tdesign-react/es/style/index.css';
import type {SelectProps} from "tdesign-react";
import * as echarts from 'echarts';
import {listen} from "@tauri-apps/api/event";
import {invoke} from "@tauri-apps/api/core";
interface name {
    data: number[];
}

interface data {
    time: number,
    num: number[],
}
interface port_info{
    port: string,
    info: string,

}
interface serial_list {
    port_vec: port_info[]
}

class App extends Component {
    state = {
        dynamicOptions: [] as SelectProps[],
        drawerVisible: false,
        dataChart: null as echarts.ECharts | null,
        port_list: [] as port_info[],
        isConnected: false,
        menuValue:0 as number
    };
    dataList: data[] = [];
    timerID: number = 0;
    bandRateList:number[] = [110,300,600,1200,2400,4800,9600,14400,19200,38400,56000,57600,115200,128000,230400,256000];
    updateChart = () => {
        if (this.state.dataChart) {
            this.state.dataChart.setOption({
                series: Array.from({length: 9}).map((_, index) => ({
                    name: `${index + 1}号传感器`,
                    type: 'line',
                    data: this.dataList?.map(d => [d.time, d.num[index]]) // 每个数据点是 [时间, 数值]
                })),
            });
        }
    };
    updatePort = (portVec: port_info[]) => {
        this.setState({port_list: portVec});
    }
    initChart = () => {
        const myChart = echarts.init(document.getElementById('main'));
        myChart.setOption({
            title: {
                text: '磁场强度',
                textAlign: 'center',
                left: 'center',
            },
            tooltip: {},
            height: "85%",
            xAxis: {
                type: 'time',
                minInterval: 10
            },
            yAxis: {
                type: 'value',
                // min: -4000000,
                // max: 4000000,
            },
            legend: {
                orient: 'vertical',
                right: 30,
                top: 20,
                bottom: 20,
            },
            dataZoom: [
                {
                    startValue: new Date().toLocaleString(),
                },
                {
                    type: 'inside'
                }
            ],
            series: Array.from({length: 9}).map((_, index) => ({
                name: `${index + 1}号传感器`,
                type: 'line',
                symbolSize: 0,
                data: this.dataList?.map(d => [d.time, d.num[index]]) // 每个数据点是 [时间, 数值]
            })),
        });
        this.setState({dataChart: myChart});
    }
    componentDidMount() {
        listen<name>("data_received", (event) => {
            this.dataList.push({num: event.payload.data, time: Date.now()})
        }).then(r => {
            return r
        })
        this.timerID = setInterval(() => {
            this.updateChart();
        }, 500);
        this.initChart();
        window.addEventListener('resize', () => {
            this.state.dataChart?.resize();
        })
    }

    componentWillUnmount() {
        // 清除定时器
        if (this.timerID) {
            clearInterval(this.timerID);
        }
    }

    render() {
        return (
            <div style={{width: "100%", height: "95vh"}}>
                <Tabs placement={'top'} size={'medium'} defaultValue={1} style={{height: "95vh"}} onChange={(value)=>{
                    if (value==1) {
                        requestAnimationFrame(() => {
                            this.initChart();
                        });
                    }
                }}>
                    <Tabs.TabPanel value={1} label="实时监测" style={{height:'95vh'}}>
                        <div id="main"
                             style={{width: "100%", height: "85%", backgroundColor: "#ffffff", borderRadius: "16px"}}></div>
                        <Button
                            shape="rectangle"
                            size="medium"
                            type="button"
                            variant="base"
                            style={{marginLeft: "auto"}}
                            onClick={async () => {
                                this.setState({drawerVisible: true});
                                (await invoke<serial_list>("get_port", {}).then(response => {
                                    this.updatePort(response.port_vec);
                                }).catch(err => {
                                    NotificationPlugin.error({
                                        title: '获取串口列表失败',
                                        content: err,
                                        placement: 'top-right',
                                        duration: 3000,
                                        offset: [0, 0],
                                        closeBtn: true,
                                    }).then(() => {
                                        console.log("打开消息通知成功");
                                    });
                                }))
                            }}
                        >
                            设置
                        </Button>
                    </Tabs.TabPanel>
                    <Tabs.TabPanel value={2} label="数据处理">
                        <p style={{ padding: 25 }}>选项卡2的内容，使用 TabPanel 渲染</p>
                    </Tabs.TabPanel>
                </Tabs>

                <Drawer header="串口设置" visible={this.state.drawerVisible}
                        onClose={() => this.setState({drawerVisible: false})}
                        footer={(
                            <div style={{textAlign: "right"}}>
                                <Button theme={"default"}
                                        onClick={() => this.setState({drawerVisible: false})}>取消</Button>
                                <Button type="submit" theme={this.state.isConnected ? "danger" : "success"}
                                        form="serialForm">
                                    {this.state.isConnected ? "停止" : "开始"}
                                </Button>
                            </div>
                        )}>
                    <Form
                        id="serialForm"
                        labelWidth={60}
                        initialData={{
                            bandRate: 9600,
                            dataBits: 8,
                            stopBits: 1,
                        }}
                        onSubmit={(m) => {
                            if (this.state.isConnected) {
                                invoke("stop_serial_task", {})
                                    .then(r => {
                                        console.log(r);
                                        this.setState({
                                            isConnected: false
                                        });
                                        NotificationPlugin.success({
                                            title: '断开成功',
                                            content: 'Success to disconnect!',
                                            placement: 'top-right',
                                            duration: 3000,
                                            offset: [0, 0],
                                            closeBtn: true,
                                        }).then(() => {
                                            console.log("打开消息通知成功");
                                        });
                                    })
                                    .catch(r => {
                                        console.log(r);
                                        NotificationPlugin.error({
                                            title: '断开失败',
                                            content: r,
                                            placement: 'top-right',
                                            duration: 3000,
                                            offset: [0, 0],
                                            closeBtn: true,
                                        }).then(() => {
                                            console.log("打开消息通知成功");
                                        });
                                    });
                            } else {
                                invoke("set_serial_cfg", m.fields)
                                    .then(r => {
                                        console.log(r);
                                        this.setState({
                                            drawerVisible: false,
                                            isConnected: true
                                        });
                                        NotificationPlugin.success({
                                            title: '连接成功',
                                            content: 'Success to connect!',
                                            placement: 'top-right',
                                            duration: 3000,
                                            offset: [0, 0],
                                            closeBtn: true,
                                        }).then(() => {
                                            console.log("打开消息通知成功");
                                        });

                                    })
                                    .catch(r => {
                                        console.log(r);
                                        NotificationPlugin.error({
                                            title: '连接失败',
                                            content: r,
                                            placement: 'top-right',
                                            duration: 3000,
                                            offset: [0, 0],
                                            closeBtn: true,
                                        }).then(() => {
                                            console.log("打开消息通知成功");
                                        });
                                    });
                            }

                            console.log(m.fields);
                        }}>
                        <Form.FormItem label="端口号" name={"portName"}>
                            <Select>
                                {this.state.port_list.map(port => {
                                    return (<Select.Option style={{ height: '60px' }} value={port.port} label={port.port}>
                                        <div style={{ marginLeft: '16px' }}>
                                            <div>{port.port}</div>
                                            <div
                                                style={{
                                                    fontSize: '13px',
                                                    color: 'var(--td-gray-color-9)',
                                                }}
                                            >
                                                {port.info}
                                            </div>
                                        </div>
                                    </Select.Option>)
                                })}
                            </Select>
                        </Form.FormItem>
                        <Form.FormItem label="波特率" name={"bandRate"}>
                            <Select
                                options={this.bandRateList.map((value) => {
                                    return ({
                                        value: value,
                                        label: value.toString()
                                    })
                                })}
                            >
                            </Select>
                        </Form.FormItem>
                        <Form.FormItem label="数据位" name={"dataBits"}>
                            <Select
                                options={[
                                    {
                                        label: "5",
                                        value: 5
                                    },
                                    {
                                        label: "6",
                                        value: 6
                                    },
                                    {
                                        label: "7",
                                        value: 7
                                    },
                                    {
                                        label: "8",
                                        value: 8
                                    },
                                ]}>
                            </Select>
                        </Form.FormItem>
                        <Form.FormItem label="停止位" name={"stopBits"}>
                            <Select
                                options={[
                                    {
                                        label: "1",
                                        value: 1
                                    },
                                    {
                                        label: "2",
                                        value: 2
                                    },
                                ]}>
                            </Select>
                        </Form.FormItem>
                    </Form>
                </Drawer>

            </div>
        );
    }
}

export default App;
