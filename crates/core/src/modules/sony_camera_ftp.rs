//! The camera's own FTP upload: its server settings list (stills cameras),
//! its transfer job list (video cameras) and its transfer counts. Layouts
//! follow the Camera Control PTP 3 Reference (SDIO_GetFTPSettingList,
//! SDIO_SetFTPSettingList, SDIO_GetFTPJobList, SDIO_ControlFTPJobList,
//! SDIO_GetDisplayFTPResult and the FTPSettingList, FTPJobList and
//! FTPResultDataset data formats).
//!
//! Passwords go to the camera only. A setting list read back carries none,
//! and nothing here puts one in a value or the state.

use serde_json::{json, Value};

use super::sony_camera_dataset::Reader;

/// The setting and job lists travel inside a wrapper: offset and size of the
/// list, then the list.
fn unwrap_list(data: &[u8]) -> Result<&[u8], String> {
    let mut r = Reader::new(data);
    let offset = r.u32()? as usize;
    let size = r.u32()? as usize;
    let end = offset.checked_add(size).ok_or("list size overflows")?;
    data.get(offset..end)
        .ok_or_else(|| "the list lies outside the dataset".to_string())
}

fn wrap_list(list: &[u8]) -> Vec<u8> {
    let mut b = 8u32.to_le_bytes().to_vec();
    b.extend_from_slice(&(list.len() as u32).to_le_bytes());
    b.extend_from_slice(list);
    b
}

fn read_text(r: &mut Reader, size: usize) -> Result<String, String> {
    if size > r.remaining() {
        return Err("text does not fit the dataset".into());
    }
    let bytes: Vec<u8> = (0..size).map(|_| r.u8()).collect::<Result<_, _>>()?;
    let end = bytes.iter().position(|c| *c == 0).unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

/// Text with its terminating NUL, preceded by a 16-bit size.
fn write_text16(b: &mut Vec<u8>, text: &str) {
    if text.is_empty() {
        b.extend_from_slice(&0u16.to_le_bytes());
        return;
    }
    let mut t = text.as_bytes().to_vec();
    t.push(0);
    b.extend_from_slice(&(t.len() as u16).to_le_bytes());
    b.extend_from_slice(&t);
}

/// Text with its terminating NUL, preceded by an 8-bit size.
fn write_text8(b: &mut Vec<u8>, text: &str) -> Result<(), String> {
    if text.is_empty() {
        b.push(0);
        return Ok(());
    }
    let mut t = text.as_bytes().to_vec();
    t.push(0);
    let len = u8::try_from(t.len()).map_err(|_| format!("'{text}' is too long"))?;
    b.push(len);
    b.extend_from_slice(&t);
    Ok(())
}

pub(crate) fn secure_name(v: u8) -> &'static str {
    match v {
        1 => "off",
        2 => "ftps",
        3 => "sftp",
        _ => "unset",
    }
}

/// One FTP server entry as the module reports it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FtpServer {
    pub id: u16,
    pub enabled: bool,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password_set: bool,
    pub passive: Option<bool>,
    pub directory: String,
    pub secure: u8,
    pub hierarchy: u8,
    pub overwrite: u8,
    pub certificate_error: u8,
}

impl FtpServer {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "server_id": self.id,
            "enabled": self.enabled,
            "name": self.name,
            "host": self.host,
            "port": self.port,
            "username": self.user,
            "password_set": self.password_set,
            "passive": self.passive,
            "directory": self.directory,
            "secure": secure_name(self.secure),
            "directory_hierarchy": match self.hierarchy { 1 => "standard", 2 => "same_as_camera", _ => "unset" },
            "overwrite": match self.overwrite { 1 => json!(true), 2 => json!(false), _ => Value::Null },
            "certificate_error": match self.certificate_error { 1 => "connect", 2 => "do_not_connect", _ => "unset" },
        })
    }
}

/// The server list and its version (100 times the version number).
pub(crate) fn parse_setting_list(data: &[u8]) -> Result<(u16, Vec<FtpServer>), String> {
    let list = unwrap_list(data)?;
    let mut r = Reader::new(list);
    let version = r.u16()?;
    r.u16()?;
    let count = r.u32()?;
    let mut servers = Vec::new();
    for _ in 0..count {
        let id = r.u16()?;
        let service = r.u8()?;
        let n = r.u16()? as usize;
        let name = read_text(&mut r, n)?;
        let n = r.u16()? as usize;
        let host = read_text(&mut r, n)?;
        let port = r.u16()?;
        let n = r.u16()? as usize;
        let user = read_text(&mut r, n)?;
        let password_set = r.u8()? == 1;
        let n = r.u16()? as usize;
        // Not sent back by the camera; skipped if it is.
        read_text(&mut r, n)?;
        let passive = match r.u8()? {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        };
        let n = r.u16()? as usize;
        let directory = read_text(&mut r, n)?;
        let secure = r.u8()?;
        let (hierarchy, overwrite, certificate_error) = if version >= 101 {
            (r.u8()?, r.u8()?, r.u8()?)
        } else {
            (0, 0, 0)
        };
        servers.push(FtpServer {
            id,
            enabled: service == 1,
            name,
            host,
            port,
            user,
            password_set,
            passive,
            directory,
            secure,
            hierarchy,
            overwrite,
            certificate_error,
        });
    }
    Ok((version, servers))
}

/// A server entry to write. A `None` password leaves the camera's as it is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FtpServerWrite {
    pub id: u16,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub passive: bool,
    pub directory: String,
    /// 1 off, 2 FTPS, 3 SFTP.
    pub secure: u8,
    /// 1 standard, 2 as in the camera.
    pub hierarchy: u8,
    /// 1 overwrite, 2 keep.
    pub overwrite: u8,
    /// 1 connect anyway, 2 do not connect.
    pub certificate_error: u8,
}

pub(crate) fn encode_setting_list(version: u16, server: &FtpServerWrite) -> Vec<u8> {
    let mut b = version.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&server.id.to_le_bytes());
    b.push(1); // FTP
    write_text16(&mut b, &server.name);
    write_text16(&mut b, &server.host);
    b.extend_from_slice(&server.port.to_le_bytes());
    write_text16(&mut b, &server.user);
    match &server.password {
        Some(p) => {
            b.push(1);
            write_text16(&mut b, p);
        }
        None => {
            b.push(0);
            b.extend_from_slice(&0u16.to_le_bytes());
        }
    }
    b.push(if server.passive { 2 } else { 1 });
    write_text16(&mut b, &server.directory);
    b.push(server.secure);
    if version >= 101 {
        b.push(server.hierarchy);
        b.push(server.overwrite);
        b.push(server.certificate_error);
    }
    wrap_list(&b)
}

pub(crate) fn job_status_name(v: u32) -> String {
    match v {
        0x0000_0100 => "waiting".into(),
        0x0000_0200 => "transferring".into(),
        0x0000_0400 => "completed".into(),
        0x0000_0800 => "aborted".into(),
        0x0001_0000 => "error".into(),
        0x0001_0001 => "authentication_error".into(),
        0x0001_0002 => "server_full".into(),
        0x0001_0003 => "source_file_unavailable".into(),
        0x0001_0004 => "invalid_server_certificate".into(),
        0x0001_0005 => "source_media_unavailable".into(),
        0x0001_0006 => "host_not_resolved".into(),
        0x0001_0007 => "server_settings_wrong".into(),
        0x0001_0008 => "upload_failed".into(),
        0x0001_0009 => "server_certificate_not_yet_valid".into(),
        0x0001_000A => "server_certificate_expired".into(),
        0x0001_000B => "server_refuses_passive_mode".into(),
        0x0001_000C => "segment_transfer_failed".into(),
        0 => "invalid".into(),
        other => format!("0x{other:08X}"),
    }
}

/// The job list: its version, sync id and jobs.
pub(crate) fn parse_job_list(data: &[u8]) -> Result<(u16, u32, Vec<Value>), String> {
    let list = unwrap_list(data)?;
    let mut r = Reader::new(list);
    let version = r.u16()?;
    r.u16()?;
    let sync_id = r.u32()?;
    let count = r.u32()?;
    let mut jobs = Vec::new();
    for _ in 0..count {
        if version >= 101 {
            r.u32()?; // size of this entry
        }
        let id = r.u32()?;
        let server = r.u32()?;
        let slot = r.u32()?;
        let status = r.u32()?;
        let chunks = r.u32()?;
        let size = r.u64()?;
        let transferred = r.u64()?;
        let n = r.u8()? as usize;
        let clip = read_text(&mut r, n)?;
        let (main, meta) = if version >= 101 {
            let n = r.u8()? as usize;
            let main = read_text(&mut r, n)?;
            let n = r.u8()? as usize;
            (main, read_text(&mut r, n)?)
        } else {
            let n = r.u8()? as usize;
            (String::new(), read_text(&mut r, n)?)
        };
        jobs.push(json!({
            "job_id": id,
            "server_id": server,
            "slot": slot,
            "status": job_status_name(status),
            "chunks": chunks,
            "size": size,
            "transferred": transferred,
            "name": clip,
            "main_file": main,
            "meta_file": meta,
        }));
    }
    Ok((version, sync_id, jobs))
}

/// A transfer job to add.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NewJob {
    pub server: u32,
    pub slot: u32,
    pub clip_path: String,
    pub directory: String,
    pub name: String,
}

pub(crate) fn encode_job_add(version: u16, job: &NewJob) -> Result<Vec<u8>, String> {
    let mut entry = Vec::new();
    entry.extend_from_slice(&job.server.to_le_bytes());
    entry.extend_from_slice(&job.slot.to_le_bytes());
    write_text8(&mut entry, &job.clip_path)?;
    entry.push(0); // no metadata path
    entry.push(0); // no TakeInfo path
    write_text16(&mut entry, &job.directory);
    if version >= 101 {
        entry.extend_from_slice(&0u32.to_le_bytes()); // in point: whole clip
        entry.extend_from_slice(&0u32.to_le_bytes()); // out point
        entry.extend_from_slice(&0u32.to_le_bytes()); // duration
        write_text8(&mut entry, &job.name)?;
        entry.push(0); // no update time
        entry.push(0); // no UMID
        entry.extend_from_slice(&0u32.to_le_bytes()); // no trimming codec
        entry.push(1);
        entry.push(1);
    }
    let mut b = version.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&1u32.to_le_bytes());
    if version >= 101 {
        b.extend_from_slice(&((entry.len() + 4) as u32).to_le_bytes());
    }
    b.extend_from_slice(&entry);
    Ok(wrap_list(&b))
}

/// The job ids a suspend, resume or delete names; `None` for all (delete).
pub(crate) fn encode_job_ids(version: u16, ids: Option<&[u32]>) -> Vec<u8> {
    let mut b = version.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0]);
    match ids {
        None => b.extend_from_slice(&0xFFFFu32.to_le_bytes()),
        Some(ids) => {
            b.extend_from_slice(&(ids.len() as u32).to_le_bytes());
            for id in ids {
                if version >= 101 {
                    b.extend_from_slice(&8u32.to_le_bytes());
                }
                b.extend_from_slice(&id.to_le_bytes());
            }
        }
    }
    wrap_list(&b)
}

/// Successful and failed transfer counts.
pub(crate) fn parse_ftp_result(data: &[u8]) -> Result<(u32, u32), String> {
    let mut r = Reader::new(data);
    r.u16()?;
    r.u16()?;
    Ok((r.u32()?, r.u32()?))
}

#[cfg(test)]
pub(crate) mod build {
    use super::*;

    /// A setting list as a camera sends it (no password).
    pub(crate) fn setting_list(version: u16, servers: &[(u16, &str, &str)]) -> Vec<u8> {
        let mut b = version.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&(servers.len() as u32).to_le_bytes());
        for (id, name, host) in servers {
            b.extend_from_slice(&id.to_le_bytes());
            b.push(1);
            write_text16(&mut b, name);
            write_text16(&mut b, host);
            b.extend_from_slice(&21u16.to_le_bytes());
            write_text16(&mut b, "cam");
            b.push(1);
            b.extend_from_slice(&0u16.to_le_bytes());
            b.push(2);
            write_text16(&mut b, "/in");
            b.push(1);
            if version >= 101 {
                b.extend_from_slice(&[1, 2, 2]);
            }
        }
        wrap_list(&b)
    }

    pub(crate) fn job_list(version: u16, sync: u32, jobs: &[(u32, u32, &str)]) -> Vec<u8> {
        let mut b = version.to_le_bytes().to_vec();
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&sync.to_le_bytes());
        b.extend_from_slice(&(jobs.len() as u32).to_le_bytes());
        for (id, status, clip) in jobs {
            let mut e = Vec::new();
            for v in [*id, 1, 1, *status, 4] {
                e.extend_from_slice(&v.to_le_bytes());
            }
            e.extend_from_slice(&1000u64.to_le_bytes());
            e.extend_from_slice(&250u64.to_le_bytes());
            write_text8(&mut e, clip).unwrap();
            if version >= 101 {
                write_text8(&mut e, clip).unwrap();
            }
            write_text8(&mut e, "").unwrap();
            if version >= 101 {
                b.extend_from_slice(&((e.len() + 4) as u32).to_le_bytes());
            }
            b.extend_from_slice(&e);
        }
        wrap_list(&b)
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn setting_lists_read_both_versions_without_passwords() {
        for version in [100, 102] {
            let (v, servers) =
                parse_setting_list(&setting_list(version, &[(1, "News", "ftp.example")])).unwrap();
            assert_eq!(v, version);
            assert_eq!(servers.len(), 1);
            let s = servers[0].to_json();
            assert_eq!(s["host"], "ftp.example");
            assert_eq!(s["password_set"], true);
            assert_eq!(s["passive"], true);
            assert!(s.get("password").is_none());
        }
    }

    #[test]
    fn a_written_server_reads_back() {
        let write = FtpServerWrite {
            id: 3,
            name: "Desk".into(),
            host: "10.0.0.9".into(),
            port: 2121,
            user: "u".into(),
            password: Some("secret".into()),
            passive: false,
            directory: "/up".into(),
            secure: 2,
            hierarchy: 1,
            overwrite: 2,
            certificate_error: 2,
        };
        let bytes = encode_setting_list(101, &write);
        let (_, servers) = parse_setting_list(&bytes).unwrap();
        assert_eq!(servers[0].host, "10.0.0.9");
        assert_eq!(servers[0].port, 2121);
        assert_eq!(servers[0].passive, Some(false));
        assert_eq!(servers[0].secure, 2);
        // The password is on the wire, NUL-terminated, after its size.
        let needle = b"secret\0";
        assert!(bytes.windows(needle.len()).any(|w| w == needle));
        // Without a password the field is marked unused.
        let keep = encode_setting_list(
            100,
            &FtpServerWrite {
                password: None,
                ..write
            },
        );
        assert!(!keep.windows(6).any(|w| w == b"secret"));
    }

    #[test]
    fn job_lists_and_job_controls() {
        for version in [100, 101] {
            let (v, sync, jobs) =
                parse_job_list(&job_list(version, 9, &[(5, 0x200, "C0001.MXF")])).unwrap();
            assert_eq!((v, sync), (version, 9));
            assert_eq!(jobs[0]["job_id"], 5);
            assert_eq!(jobs[0]["status"], "transferring");
            assert_eq!(jobs[0]["transferred"], 250);
        }
        let add = encode_job_add(
            101,
            &NewJob {
                server: 1,
                slot: 1,
                clip_path: "/Clip/C0001.MXF".into(),
                directory: String::new(),
                name: String::new(),
            },
        )
        .unwrap();
        assert_eq!(&add[..8], &[8, 0, 0, 0, (add.len() - 8) as u8, 0, 0, 0]);
        let all = encode_job_ids(101, None);
        assert_eq!(&all[8..], &[101, 0, 0, 0, 0xFF, 0xFF, 0, 0]);
        let one = encode_job_ids(101, Some(&[5]));
        assert_eq!(&one[12..], &[1, 0, 0, 0, 8, 0, 0, 0, 5, 0, 0, 0]);
        assert_eq!(
            parse_ftp_result(&[100, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0]).unwrap(),
            (3, 1)
        );
    }
}
