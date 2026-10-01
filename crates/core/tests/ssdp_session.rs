//! SSDP discovery of a Sony camera through the public API. A core bound to
//! 127.0.0.1 scans with a hint; a simulated camera at 127.0.0.2 answers the
//! unicast M-SEARCH on UDP 1900 and serves dd.xml and DigitalImagingDesc.xml
//! over HTTP. Multicast is not used: loopback multicast is unreliable across
//! the platforms the tests run on.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meros_integrations::{Core, CoreOptions, DiscoverAction, DiscoverRequest, Event};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};

const URN: &str = "urn:schemas-sony-com:service:DigitalImaging:1";
const CAMERA: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

const DD_XML: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:Basic:1</deviceType>
    <friendlyName>Camera A</friendlyName>
    <manufacturer>Sony Corporation</manufacturer>
    <modelName>ILCE-7M4</modelName>
    <UDN>uuid:00000000-0000-0010-8000-0000000000a1</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-sony-com:service:DigitalImaging:1</serviceType>
        <serviceId>urn:schemas-sony-com:serviceId:DigitalImaging</serviceId>
        <SCPDURL>DigitalImagingDesc.xml</SCPDURL>
      </service>
    </serviceList>
  </device>
</root>"#;

const IMAGING_XML: &str = r#"<?xml version="1.0"?>
<scpd xmlns="urn:schemas-upnp-org:service-1-0" xmlns:x="urn:schemas-sony-com:x">
  <x:X_ModelName>ILCE-7M4</x:X_ModelName>
  <x:X_PTP_Versions>3.00</x:X_PTP_Versions>
  <x:X_PTP_PairingNecessity>Disable</x:X_PTP_PairingNecessity>
  <x:X_SSH_Support>Enable</x:X_SSH_Support>
</scpd>"#;

async fn serve_descriptions() -> u16 {
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(CAMERA), 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut got = Vec::new();
                let mut buf = [0u8; 2048];
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => got.extend_from_slice(&buf[..n]),
                    }
                }
                let body = if got.starts_with(b"GET /dd.xml") {
                    DD_XML
                } else {
                    IMAGING_XML
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/xml\r\ncontent-length: {}\r\n\
                     connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    port
}

/// UDP 1900 on 127.0.0.2, shared as the core's listener shares it.
fn camera_socket() -> UdpSocket {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
        .bind(&SocketAddr::new(IpAddr::V4(CAMERA), 1900).into())
        .unwrap();
    UdpSocket::from_std(socket.into()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sony_camera_answers_a_search_and_is_described() {
    let http_port = serve_descriptions().await;
    let camera = camera_socket();
    let location = format!("http://127.0.0.2:{http_port}/dd.xml");
    tokio::spawn(async move {
        let mut buf = [0u8; 2048];
        loop {
            let Ok((n, from)) = camera.recv_from(&mut buf).await else {
                continue;
            };
            let request = String::from_utf8_lossy(&buf[..n]);
            if !request.starts_with("M-SEARCH") || !request.contains(URN) {
                continue;
            }
            let answer = format!(
                "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nEXT:\r\nLOCATION: {location}\r\n\
                 SERVER: UPnP/1.0 SonyImagingDevice/1.0\r\nST: {URN}\r\n\
                 USN: uuid:00000000-0000-0010-8000-0000000000a1::{URN}\r\n\r\n"
            );
            let _ = camera.send_to(answer.as_bytes(), from).await;
        }
    });

    let core = Core::with_options(CoreOptions {
        bind_address: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
    })
    .unwrap();
    core.discover(DiscoverRequest {
        action: DiscoverAction::Scan,
        protocols: vec!["ssdp".into()],
        hints: vec![CAMERA],
    })
    .unwrap();

    let found = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            for e in core.next_events(64).await {
                if let Event::Discovered { .. } = e {
                    return e;
                }
            }
        }
    })
    .await
    .expect("discovered within 10 s");
    let Event::Discovered {
        protocol,
        address,
        port,
        device,
        models,
        name,
        evidence,
    } = found
    else {
        unreachable!()
    };
    assert_eq!(protocol, "ssdp");
    assert_eq!(address, "127.0.0.2");
    assert_eq!(port, 15740);
    assert_eq!(device, "sony-camera");
    assert_eq!(models, ["ilce-7m4"]);
    assert_eq!(name.as_deref(), Some("Camera A"));
    assert_eq!(
        evidence["usn"],
        format!("uuid:00000000-0000-0010-8000-0000000000a1::{URN}")
    );
    assert_eq!(evidence["model_name"], "ILCE-7M4");
    assert_eq!(evidence["ptp3"], true);
    assert_eq!(evidence["pairing"], false);
    assert_eq!(evidence["ssh"], true);
    assert_eq!(evidence["suggested_connection"], "ssh");

    // Three M-SEARCH rounds, one camera: reported once.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let again = core
        .poll_events(64)
        .into_iter()
        .filter(|e| matches!(e, Event::Discovered { .. }))
        .count();
    assert_eq!(again, 0, "a known camera is not reported again");

    core.discover(DiscoverRequest {
        action: DiscoverAction::Stop,
        protocols: vec!["ssdp".into()],
        hints: vec![],
    })
    .unwrap();
}
