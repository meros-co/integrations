//! vMix simulated on real TCP, driven through the public API.
//!
//! The simulator sends the unrequested VERSION line, answers SUBSCRIBE, XML
//! (with the byte count the TCP API specifies), ACTS queries and FUNCTION,
//! and pushes a TALLY event after a cut, before the FUNCTION reply, as the
//! API allows. Its XML has the shape vMix 27 reports: inputs with audio, a
//! GT title, layers, a replay input, the audio buses, mixes, outputs and the
//! dynamic values.

#![cfg(feature = "vmix")]

use std::net::SocketAddr;
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const XML: &str = r#"<vmix><version>27.0.0.49</version><edition>4K</edition><preset>C:\Shows\Sunday.vmix</preset><inputs><input key="a" number="1" type="Capture" title="Camera 1" shortTitle="Camera 1" state="Running" position="0" duration="0" loop="False" muted="False" volume="100" balance="0" solo="False" soloPFL="False" audiobusses="M,A" meterF1="0.0912" meterF2="0.0837" gainDb="0">Camera 1</input><input key="b" number="2" type="Capture" title="Camera 2" shortTitle="Camera 2" state="Running" position="0" duration="0" loop="False">Camera 2</input><input key="c" number="3" type="GT" title="Lower third.gtzip" shortTitle="Lower third" state="Paused" position="0" duration="0" loop="False" selectedIndex="0">Lower third.gtzip<text index="0" name="Headline.Text">Hello</text></input><input key="d" number="4" type="Colour" title="PiP" shortTitle="PiP" state="Paused" position="0" duration="0" loop="False">PiP<overlay index="0" key="b"><position panX="-0.5" panY="0.5" zoomX="0.5" zoomY="0.5"/></overlay></input><input key="e" number="5" type="Replay" title="Replay" shortTitle="Replay" state="Running" position="0" duration="0" loop="False">Replay<replay live="True" recording="True" channelMode="AB" events="1" eventsA="1" eventsB="1" cameraA="1" cameraB="2" speed="1" speedA="1" speedB="1"><timecode>2026-10-07T10:15:30.120</timecode><timecodeA>2026-10-07T10:15:30.120</timecodeA><timecodeB>2026-10-07T10:15:30.120</timecodeB></replay></input></inputs><overlays><overlay number="1">3</overlay><overlay number="2"/><overlay number="3"/><overlay number="4"/></overlays><preview>2</preview><active>1</active><fadeToBlack>False</fadeToBlack><transitions><transition number="1" effect="Fade" duration="500"/><transition number="2" effect="Merge" duration="1000"/><transition number="3" effect="Wipe" duration="500"/><transition number="4" effect="CubeZoom" duration="1000"/></transitions><recording duration="12">True</recording><external>False</external><streaming channel1="False" channel2="False" channel3="False">False</streaming><playList>False</playList><multiCorder>False</multiCorder><fullscreen>False</fullscreen><mix number="2"><preview>4</preview><active>3</active></mix><outputs><output type="Output" number="2" source="Input" inputNumber="4" ndi="False" omt="False" srt="False"/></outputs><audio><master volume="100" muted="False" meterF1="0.0844" meterF2="0.0799" headphonesVolume="74"/><busA volume="80" muted="False" meterF1="0" meterF2="0" solo="False" sendToMaster="False"/></audio><dynamic><input1>Camera 1</input1><input2/><input3/><input4/><value1>Hello</value1><value2/><value3/><value4/></dynamic></vmix>"#;

async fn simulated_vmix() -> u16 {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let (read, mut write) = tcp.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"VERSION OK 27.0.0.49\r\n").await.unwrap();
        while let Ok(Some(line)) = lines.next_line().await {
            let reply = if let Some(what) = line.strip_prefix("SUBSCRIBE ") {
                format!("SUBSCRIBE OK {what}\r\n")
            } else if line == "XML" {
                format!("XML {}\r\n{XML}\r\n", XML.len() + 2)
            } else if line == "ACTS InputAudioAuto 1" {
                "ACTS OK InputAudioAuto 1 1\r\n".to_string()
            } else if let Some(query) = line.strip_prefix("ACTS ") {
                // Every channel at half volume.
                format!("ACTS OK {query} 0.5\r\n")
            } else if line == "FUNCTION Cut" {
                // Input 2 goes to program and 1 to preview; the tally event
                // arrives before the reply.
                "TALLY OK 21000\r\nFUNCTION OK Completed\r\n".to_string()
            } else if line.starts_with("FUNCTION ") {
                "FUNCTION ER Unknown function\r\n".to_string()
            } else {
                continue;
            };
            write.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    port
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event within 10 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn vmix_end_to_end() {
    let port = simulated_vmix().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "vmix".into(),
            model: "vmix".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"state_poll_ms": 60000})),
            monitor: true,
        })
        .unwrap();

    // The last channel mixer answer comes after the XML state.
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["inputs"]["1"]["channel_mixer"]["16"] == 50.0)
    })
    .await;
    let snapshot = core.snapshot(id).unwrap();
    assert_eq!(snapshot.connection, Connection::Connected);
    let s = &snapshot.state;
    assert_eq!(s["program"], 1);
    assert_eq!(s["device"]["edition"], "4K");
    assert_eq!(s["device"]["preset"], r"C:\Shows\Sunday.vmix");
    assert_eq!(s["inputs"]["1"]["audio_buses"]["A"], true);
    assert_eq!(s["inputs"]["1"]["audio_auto"], true);
    assert_eq!(s["inputs"]["1"]["meter_left"], 0.0912);
    assert_eq!(s["inputs"]["1"]["attributes"]["soloPFL"], "False");
    assert_eq!(s["inputs"]["3"]["texts"]["0"]["value"], "Hello");
    assert_eq!(s["inputs"]["4"]["layers"]["1"]["input"], 2);
    assert_eq!(s["inputs"]["4"]["layers"]["1"]["transform"]["zoom_x"], 0.5);
    assert_eq!(s["overlays"]["1"]["input"], 3);
    assert_eq!(s["transitions"]["2"]["effect"], "Merge");
    assert_eq!(s["mixes"]["2"]["program"], 3);
    assert_eq!(s["outputs"]["output2"]["input"], 4);
    assert_eq!(s["master"]["headphones_volume"], 74.0);
    assert_eq!(s["buses"]["A"]["volume"], 80.0);
    assert_eq!(s["replay"]["input"], 5);
    assert_eq!(s["replay"]["camera_b"], 2);
    assert_eq!(s["dynamic"]["values"]["1"], "Hello");
    assert_eq!(s["recording_duration"], 12);
    // Inputs without audio are not asked about.
    assert_eq!(s["inputs"]["2"].get("channel_mixer"), None);

    assert_eq!(
        core.execute(id, "cut", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    let tally = &core.snapshot(id).unwrap().state["tally"];
    assert_eq!(tally["1"]["preview"], true);
    assert_eq!(tally["2"]["program"], true);

    let outcome = core.execute(id, "fade_to_black", params(json!({}))).await;
    assert!(matches!(outcome, Err(CommandError::DeviceError { .. })));

    core.close(id).await;
}
