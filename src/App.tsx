import {Component} from "react";
// import {invoke} from "@tauri-apps/api/core";
// import {Tabs} from "tdesign-react";
import './App.css'
import 'tdesign-react/es/style/index.css';
// import DataAnalysis from "./DataAnalysis.tsx";
import RealTime from "./RealTime.tsx";


class App extends Component {
    state = {};


    componentDidMount() {

    }


    render() {
        return (
            <div style={{width: "100%", height: "100vh"}}>
                {/*<Tabs placement={'top'} size={'medium'} defaultValue={1} style={{height: "100%"}}>*/}
                {/*<Tabs.TabPanel value={1} label="实时监测">*/}
                <RealTime></RealTime>
                {/*</Tabs.TabPanel>*/}
                {/*<Tabs.TabPanel value={2} label="数据处理">*/}
                {/*<DataAnalysis></DataAnalysis>*/}
                {/*    </Tabs.TabPanel>*/}
                {/*</Tabs>*/}


            </div>
        );
    }
}

export default App;
