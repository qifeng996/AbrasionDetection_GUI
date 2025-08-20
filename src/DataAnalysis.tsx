import {Component} from "react";
import {Button, DateRangePicker, NotificationPlugin} from "tdesign-react";
import {invoke} from "@tauri-apps/api/core";
import * as echarts from "echarts";
import dayjs from "dayjs";

interface db_data {
    id: number,
    time: string,
    data1: number,
    data2: number,
    data3: number,
    data4: number,
    data5: number,
    data6: number,
    data7: number,
    data8: number,
    data9: number,
}

interface RadData {
    id: number,
    time: string,
    num: number[],
}

class DataAnalysis extends Component {
    state = {
        dateRangeValue: [] as string[],
        radChart: null as echarts.ECharts | null,
    }
    radData: RadData[] = [];

    componentDidMount() {
        this.initChart();
        window.addEventListener('resize', () => {
            this.state.radChart?.resize();
        })
    }

    initChart = () => {
        let myChart = echarts.init(document.getElementById('chart2'));
        myChart.setOption({
            polar: {
                radius: [30, '80%']
            },
            radiusAxis: {
                inverse: false,
                min: -4000000,
                max: 4000000
            },
            angleAxis: {
                type: 'category',
                data: [],
                startAngle: -105
            },
            tooltip: {},
            series: Array.from({length: 9}).map((_, index) => ({
                name: `${index + 1}号传感器`,
                type: 'line',
                data: [],
                coordinateSystem: 'polar',
                symbolSize: 5,
            })),
            legend: {
                orient: 'vertical',
                left: 30,
                top: 20,
                bottom: 20,
            },
            animation: true,
        })
        this.setState({radChart: myChart})
    }
    updateChart = () => {
        if (this.state.radChart) {
            this.state.radChart.setOption({
                series: Array.from({length: 9}).map((_, index) => ({
                    name: `${index + 1}号传感器`,
                    type: 'line',
                    data: [...this.radData?.map(d => d.num[index]), this.radData[0].num[index]],
                    coordinateSystem: 'polar',
                    symbolSize: 1,
                    smooth: true
                })),
                angleAxis: {
                    type: 'category',
                    data: this.radData.map(v => dayjs(v.time).format("HH:mm:ss.SSS")),
                    startAngle: -105
                },
                radiusAxis: {
                    inverse: true
                },
            })

        }
    }

    render() {
        return (
            <div
                style={{
                    height: '95vh',
                    width: '100%',
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center'
                }}
            >
                <div id="chart2"
                     style={{
                         width: "70%",
                         height: "85%",
                         backgroundColor: "#ffffff",
                         borderRadius: "16px"
                     }}></div>
                <div style={{
                    width: "25%",
                    height: "85%"
                }}>
                    <DateRangePicker
                        value={this.state.dateRangeValue}
                        onChange={(value) => {
                            this.setState({dateRangeValue: value});
                        }}
                        style={{marginTop: "10px", width: '100%'}}
                        enableTimePicker/>

                    <Button
                        block
                        shape="rectangle"
                        size="medium"
                        type="button"
                        variant="base"
                        style={{marginLeft: "auto", marginTop: "10px"}}
                        onClick={() => {
                            invoke<db_data[]>("get_data_by_time", {
                                begin: this.state.dateRangeValue[0],
                                end: this.state.dateRangeValue[1]
                            }).then(response => {
                                this.radData = response.map(v => {
                                    return {
                                        time: v.time,
                                        num: [v.data1, v.data2, v.data3, v.data4, v.data5, v.data6, v.data7, v.data8, v.data9],
                                        id: v.id
                                    }
                                });
                                this.updateChart();
                                console.log(response)
                            }).catch(err => {
                                console.log(err)
                            });
                        }}
                    >
                        连接数据库
                    </Button>
                    <Button
                        block
                        shape="rectangle"
                        size="medium"
                        type="button"
                        variant="base"
                        style={{marginLeft: "auto", marginTop: "10px"}}
                        onClick={() => {
                            invoke<string>("gen_xlsx", {
                                begin: this.state.dateRangeValue[0],
                                end: this.state.dateRangeValue[1]
                            }).then(response => {
                                NotificationPlugin.success({
                                    title: '数据导出成功',
                                    content: response,
                                    placement: 'top-right',
                                    duration: 3000,
                                    offset: [0, 0],
                                    closeBtn: true,
                                }).finally();
                            }).catch(err => {
                                NotificationPlugin.error({
                                    title: '数据导出失败',
                                    content: err,
                                    placement: 'top-right',
                                    duration: 3000,
                                    offset: [0, 0],
                                    closeBtn: true,
                                }).finally();
                            });
                        }}
                    >
                        导出数据
                    </Button>
                </div>
            </div>
        );
    }
}

export default DataAnalysis;