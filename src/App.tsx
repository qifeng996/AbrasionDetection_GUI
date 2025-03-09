import {Component} from "react";
// import {invoke} from "@tauri-apps/api/core";
import {Button, Drawer, Form, NotificationPlugin, Select, SelectOption} from "tdesign-react";
import './App.css'
import 'tdesign-react/es/style/index.css';
import type {SelectProps} from "tdesign-react";
import * as echarts from 'echarts';
import {listen} from "@tauri-apps/api/event";
import {invoke} from "@tauri-apps/api/core";
import {appConfigDir} from "@tauri-apps/api/path"

interface name {
    data: number[];
}

interface data {
    time: number,
    num: number[],
}

interface serial_list {
    port_vec: string[]
}

class App extends Component {
    state = {
        dynamicOptions: [] as SelectProps[],
        drawerVisible: false,
        dataChart: null as echarts.ECharts | null,
        port_list: [] as SelectOption[],
        isConnected: false,
    };
    dataList: data[] = [];
    timerID: number = 0;

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
    updatePort = (portVec: SelectOption[]) => {
        this.setState({port_list: portVec});
    }

    componentDidMount() {
        listen<name>("data_received", (event) => {
            this.dataList.push({num: event.payload.data, time: Date.now()});
            // this.updateChart();

        }).then(r => {
            return r
        })
        listen<serial_list>("serial_change", (event) => {
            let port = [] as SelectOption[];
            for (const port_info of event.payload.port_vec) {
                port.push({
                    value: port_info,
                    label: port_info
                })
            }
            this.updatePort(port);
            console.log(port);

        }).then(UnlistenFn => {
            return UnlistenFn;
        });
        this.timerID = setInterval(() => {
            this.updateChart();
        }, 500);
        const options: SelectProps[] = [];
        for (let i = 0; i < 20; i++) {
            options.push({label: "选项" + (i + 1), value: i})
        }
        this.setState({dynamicOptions: options});
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
                right: 10,
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
        window.addEventListener('resize', () => {
            myChart.resize();
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
                <div id="main"
                     style={{width: "100%", height: "95%", backgroundColor: "#ffffff", borderRadius: "16px"}}></div>
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
                        <Form.FormItem label="端口号" name={"portName"} rules={[{required: true}]}>
                            <Select
                                options={this.state.port_list}>
                            </Select>
                        </Form.FormItem>
                        <Form.FormItem label="波特率" name={"bandRate"} rules={[{required: true}]}>
                            <Select
                                options={[
                                    {
                                        label: "9600",
                                        value: 9600
                                    },
                                    {
                                        label: "115200",
                                        value: 115200
                                    },
                                ]}>
                            </Select>
                        </Form.FormItem>
                        <Form.FormItem label="数据位" name={"dataBits"} rules={[{required: true}]}>
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
                        <Form.FormItem label="停止位" name={"stopBits"} rules={[{required: true}]}>
                            <Select
                                options={[
                                    {
                                        label: "1",
                                        value: 1
                                    },
                                    {
                                        label: "1.5",
                                        value: 1.5
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
                <Button
                    shape="rectangle"
                    size="medium"
                    type="button"
                    variant="base"
                    style={{marginLeft: "auto"}}
                    onClick={async () => {
                        this.setState({drawerVisible: true});
                        let a = await appConfigDir();
                        console.log(a)
                        invoke("greet", {name: "helloworld"});
                    }}
                >
                    设置
                </Button>
            </div>
        );
    }
}

export default App;
