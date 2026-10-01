//! File transfers to and from the camera that are not card content: LUT
//! (cube file) and scene file import, custom grid line import, the camera's
//! own setting files both ways (camera settings, FTP settings), the general
//! setting XML, scene file export, and content deletion.
//!
//! The sequences follow Sony's Camera Control PTP 3 Reference: Tips, "Import
//! (Upload) the Cube File (LUT data)" and "CameraSetting File"; Operations,
//! SDIO_UploadData, SDIO_UploadPartialData, SDIO_ControlUploadData,
//! SDIO_DownloadData, SDIO_ControlGeneralSettingFile,
//! SDIO_GetControlGeneralSettingResultFile, SDIO_SetFTPSettingFilePassword,
//! SendObject and GetObject with the setting-file handles, SDIO_DeleteContent;
//! and the events that report each result.
//!
//! An upload reads the host file whole, checks it, sends it (in parts where
//! the reference uses partial uploads), asks the camera to apply it, and
//! completes with the result event the camera sends afterwards. Every step is
//! refused first when the camera's enable-status properties say it cannot do
//! it now.

use super::*;
use ds::Reader;

pub(super) const READ: Key = "upload";
pub(super) const UPLOAD_WAIT: Key = "upload-result";
pub(super) const DELETE_WAIT: Key = "delete-result";

const OP_SEND_OBJECT: u16 = 0x100D;
const OP_SET_FTP_SETTING_FILE_PASSWORD: u16 = 0x920F;
const OP_UPLOAD_DATA: u16 = 0x921A;
const OP_CONTROL_UPLOAD_DATA: u16 = 0x921B;
const OP_DOWNLOAD_DATA: u16 = 0x921D;
const OP_CONTROL_GENERAL_SETTING_FILE: u16 = 0x9221;
const OP_GET_CONTROL_GENERAL_SETTING_RESULT_FILE: u16 = 0x9222;
const OP_UPLOAD_PARTIAL_DATA: u16 = 0x9229;
const OP_DELETE_CONTENT: u16 = 0x9250;

pub(super) const CAMERA_SETTING_HANDLE: u32 = 0xFFFF_C004;
pub(super) const FTP_SETTING_HANDLE: u32 = 0xFFFF_C005;

const DATA_CUBE: u32 = 0x0002_0001;
const DATA_SCENE: u32 = 0x0007_0001;
const DATA_GRID_LINE: u32 = 0x0009_0001;
const CONTROL_BASE_LOOK: u32 = 0x0002_0000;
const CONTROL_SCENE: u32 = 0x0007_0000;
const CONTROL_GRID_LINE: u32 = 0x0009_0000;

/// How long the camera may take to report an applied upload or a deletion.
const RESULT_TIMEOUT: Millis = 120_000;
const DELETE_TIMEOUT: Millis = 30_000;
/// The reference asks for this much time between deletions.
const DELETE_SPACING: Millis = 600;
/// Uploads are read whole; nothing documented comes near this.
const MAX_UPLOAD: u64 = 64 * 1024 * 1024;
/// A custom grid line file of dataset version 1.00 is at most 1 MB.
const MAX_GRID_LINE: u64 = 1024 * 1024;
const MAX_FILE_NAME: usize = 255;

const ENABLED: PtpValue = PtpValue::Int(1);

#[derive(Debug, Clone, PartialEq)]
pub(super) enum UploadKind {
    /// A cube file into UserBaseLook 1 to 16.
    Lut {
        index: u16,
    },
    /// A scene file into Scene 1 to 16.
    Scene {
        index: u32,
    },
    /// A custom grid line file into Custom 1 to 4.
    GridLine {
        index: u8,
    },
    CameraSettings,
    FtpSettings {
        password: String,
    },
    /// The general setting XML: 1 check, 2 apply.
    General {
        control: u32,
    },
}

impl UploadKind {
    fn name(&self) -> &'static str {
        match self {
            UploadKind::Lut { .. } => "lut",
            UploadKind::Scene { .. } => "scene_file",
            UploadKind::GridLine { .. } => "grid_line_file",
            UploadKind::CameraSettings => "camera_settings",
            UploadKind::FtpSettings { .. } => "ftp_settings",
            UploadKind::General { .. } => "general_settings",
        }
    }

    /// The SDIO_ControlUploadData control type whose result event ends it.
    fn control_type(&self) -> Option<u32> {
        match self {
            UploadKind::Lut { .. } => Some(CONTROL_BASE_LOOK),
            UploadKind::Scene { .. } => Some(CONTROL_SCENE),
            UploadKind::GridLine { .. } => Some(CONTROL_GRID_LINE),
            _ => None,
        }
    }

    /// The camera's result code in words.
    fn result_name(&self, result: u32) -> String {
        let named = match (self, result) {
            (_, 1) => "ok",
            (UploadKind::Lut { .. }, 2) => "failed",
            (UploadKind::Lut { .. }, 3) => "invalid_file_name",
            (UploadKind::Lut { .. }, 4) => "camera_busy",
            (UploadKind::GridLine { .. }, 2) => "camera_status_error",
            (UploadKind::GridLine { .. }, 3) => "invalid_file_name",
            (UploadKind::GridLine { .. }, 4) => "media_error",
            (UploadKind::GridLine { .. }, 5) => "read_error",
            (UploadKind::GridLine { .. }, 6) => "file_too_large",
            (UploadKind::GridLine { .. }, 7) => "image_too_large",
            (UploadKind::GridLine { .. }, 8) => "decode_error",
            (UploadKind::GridLine { .. }, 9) => "invalid_parameter",
            (UploadKind::FtpSettings { .. }, 3) => "wrong_password",
            (UploadKind::General { .. }, 2) => "file_error",
            (UploadKind::General { .. }, 3) => "camera_state_error",
            (UploadKind::General { .. }, 4) => "other_error",
            (UploadKind::General { .. }, 5) => "canceled",
            (_, 0) => "invalid",
            (_, 2) => "failed",
            _ => return format!("result {result}"),
        };
        named.to_string()
    }
}

pub(super) struct Upload {
    pub id: CommandId,
    pub kind: UploadKind,
    /// The file name the camera is told, from the host path or given.
    pub name: String,
    /// The last operation has been answered; the result event is awaited.
    pub awaiting: bool,
    /// A result event that arrived before that answer.
    pub result: Option<u32>,
}

pub(super) struct Deletion {
    pub id: CommandId,
    pub content: u32,
    pub slot: u32,
    pub awaiting: bool,
    pub result: Option<u32>,
}

fn delete_result_name(v: u32) -> &'static str {
    match v {
        1 => "ok",
        2 => "camera_status_error",
        3 => "camera_busy",
        4 => "protected",
        5 => "not_found",
        _ => "invalid",
    }
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn nul_terminated(text: &str) -> Vec<u8> {
    let mut b = text.as_bytes().to_vec();
    b.push(0);
    b
}

/// SDIUploadDataset: version, offset and size of the header (the file name),
/// offset and size of the file, then header and file.
pub(super) fn upload_dataset(version: u32, name: &str, file: &[u8]) -> Vec<u8> {
    let header = nul_terminated(name);
    let header_at = 20u32;
    let file_at = header_at + header.len() as u32;
    let mut b = version.to_le_bytes().to_vec();
    b.extend_from_slice(&header_at.to_le_bytes());
    b.extend_from_slice(&(header.len() as u32).to_le_bytes());
    b.extend_from_slice(&file_at.to_le_bytes());
    b.extend_from_slice(&(file.len() as u32).to_le_bytes());
    b.extend_from_slice(&header);
    b.extend_from_slice(file);
    b
}

/// SDIUploadPartialDataset (version 1.00): version, offset and size of the
/// part, then the part.
pub(super) fn partial_dataset(part: &[u8]) -> Vec<u8> {
    let mut b = 100u32.to_le_bytes().to_vec();
    b.extend_from_slice(&12u32.to_le_bytes());
    b.extend_from_slice(&(part.len() as u32).to_le_bytes());
    b.extend_from_slice(part);
    b
}

/// The file inside a DownloadDataset or a general-setting result: an
/// offset-and-size header in front of the bytes.
fn embedded_file(data: &[u8], header_words: usize) -> Result<&[u8], String> {
    let mut r = Reader::new(data);
    for _ in 0..header_words - 2 {
        r.u32()?;
    }
    let offset = r.u32()? as usize;
    let size = r.u32()? as usize;
    let end = offset.checked_add(size).ok_or("size overflows")?;
    data.get(offset..end)
        .ok_or_else(|| "the file lies outside the dataset".to_string())
}

impl SonyCamera {
    /// Refused unless the camera reports `code` as enabled (value 1), or does
    /// not report it at all.
    fn require_enabled(&self, code: u16, what: &str) -> Result<(), CommandError> {
        match self.props.get(&code) {
            Some(info) if info.current != ENABLED || info.enabled == Enabled::No => Err(refused(
                "not_available",
                format!("the camera cannot {what} now (it reports property 0x{code:04X} disabled)"),
            )),
            _ => Ok(()),
        }
    }

    /// A command-version property's value, which the camera reports only
    /// while the feature is usable; `display_only` is what the reference
    /// requires of BaseLookImport Command Version.
    fn command_version(
        &self,
        code: u16,
        what: &str,
        display_only: bool,
    ) -> Result<u32, CommandError> {
        let info = self.props.get(&code).ok_or_else(|| {
            refused(
                "not_reported",
                format!("the camera does not report 0x{code:04X}: it cannot {what}"),
            )
        })?;
        let usable = if display_only {
            info.enabled == Enabled::DisplayOnly
        } else {
            info.enabled != Enabled::No
        };
        if !usable {
            return Err(refused(
                "not_available",
                format!("the camera cannot {what} now (0x{code:04X} is not enabled)"),
            ));
        }
        Ok(info.current.as_int().unwrap_or(100) as u32)
    }

    fn largest_dataset(&self) -> Option<u64> {
        match self.props.get(&0xD0C1).map(|p| &p.form) {
            Some(Form::Range { max, .. }) => max.as_int().map(|m| m as u64),
            _ => None,
        }
        .or_else(|| {
            self.props
                .get(&0xD0C1)
                .and_then(|p| p.current.as_int())
                .map(|m| m as u64)
        })
        .filter(|m| *m > 16)
    }

    fn ftp_password(params: &Params) -> Result<String, CommandError> {
        let password = params.get("password").and_then(Value::as_str).unwrap_or("");
        if password.len() > 32 || !password.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(invalid(
                "the FTP-setting file password is up to 32 letters and digits",
            ));
        }
        Ok(password.to_string())
    }

    fn password_op(password: &str, id: CommandId) -> Op {
        let mut op = Op::with_data(
            OP_SET_FTP_SETTING_FILE_PASSWORD,
            vec![1],
            nul_terminated(password),
        )
        .for_command(id);
        op.step = Step::Prelude;
        op
    }

    fn upload(
        &self,
        id: CommandId,
        kind: UploadKind,
        params: &Params,
        max: u64,
    ) -> Result<Plan, CommandError> {
        if self.upload.is_some() {
            return Err(refused("busy", "another upload is in progress"));
        }
        let path = param_str(params, "path")?.to_string();
        let name = match params.get("name").and_then(Value::as_str) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => file_name(&path),
        };
        if name.len() > MAX_FILE_NAME {
            return Err(invalid("the file name is longer than 255 bytes"));
        }
        Ok(Plan::Upload(
            Upload {
                id,
                kind,
                name,
                awaiting: false,
                result: None,
            },
            path,
            max,
        ))
    }

    fn download_handle(
        &self,
        id: CommandId,
        params: &Params,
        handle: u32,
        prelude: Option<Op>,
    ) -> Result<Plan, CommandError> {
        if self.download.is_some() {
            return Err(refused("busy", "another download is in progress"));
        }
        let d = Download {
            id,
            path: param_str(params, "path")?.to_string(),
            source: Source::Handle(handle),
            offset: 0,
            total: None,
            name: None,
            error: None,
            closing: false,
        };
        let info = Op::new(OP_GET_OBJECT_INFO, vec![handle], Step::DownloadInfo).for_command(id);
        Ok(Plan::Download(
            d,
            prelude.into_iter().chain([info]).collect(),
        ))
    }

    /// Commands of this part of the module; `None` for any other name.
    pub(super) fn plan_files(
        &self,
        name: &str,
        params: &Params,
        id: CommandId,
    ) -> Option<Result<Plan, CommandError>> {
        let index = |name: &str, max: i128| -> Result<i128, CommandError> {
            let v = param_int(params, name)?;
            if !(1..=max).contains(&v) {
                return Err(invalid(format!("'{name}' must be 1 to {max}")));
            }
            Ok(v)
        };
        Some((|| -> Result<Plan, CommandError> {
            match name {
                "import_lut" => {
                    self.require_enabled(0xD08B, "import a LUT")?;
                    self.command_version(0xD059, "import a LUT", true)?;
                    let index = index("user_base_look", 16)? as u16;
                    self.upload(
                        id,
                        UploadKind::Lut { index },
                        params,
                        self.largest_dataset().unwrap_or(MAX_UPLOAD),
                    )
                }
                "import_scene_file" => {
                    self.command_version(0xE0E6, "import a scene file", false)?;
                    self.require_enabled(0xE101, "import a scene file")?;
                    let index = index("scene", 16)? as u32;
                    self.upload(
                        id,
                        UploadKind::Scene { index },
                        params,
                        self.largest_dataset().unwrap_or(MAX_UPLOAD),
                    )
                }
                "import_grid_line_file" => {
                    self.command_version(0xE110, "import a grid line file", false)?;
                    let index = index("custom", 4)? as u8 + 0x10;
                    self.upload(id, UploadKind::GridLine { index }, params, MAX_GRID_LINE)
                }
                "import_camera_settings" => {
                    self.require_enabled(0xD272, "read a camera-setting file")?;
                    self.upload(id, UploadKind::CameraSettings, params, MAX_UPLOAD)
                }
                "import_ftp_settings_file" => {
                    self.require_enabled(0xD275, "read an FTP-setting file")?;
                    let password = Self::ftp_password(params)?;
                    self.upload(id, UploadKind::FtpSettings { password }, params, MAX_UPLOAD)
                }
                "apply_settings_file" => {
                    self.require_enabled(0xE081, "take a general setting file")?;
                    let control = if param_bool(params, "check_only").unwrap_or(false) {
                        1
                    } else {
                        2
                    };
                    self.upload(id, UploadKind::General { control }, params, MAX_UPLOAD)
                }
                "export_camera_settings" => {
                    self.require_enabled(0xD271, "save a camera-setting file")?;
                    self.download_handle(id, params, CAMERA_SETTING_HANDLE, None)
                }
                "export_ftp_settings_file" => {
                    self.require_enabled(0xD274, "save an FTP-setting file")?;
                    let password = Self::ftp_password(params)?;
                    self.download_handle(
                        id,
                        params,
                        FTP_SETTING_HANDLE,
                        Some(Self::password_op(&password, id)),
                    )
                }
                "export_scene_file" => {
                    self.command_version(0xE0E6, "export a scene file", false)?;
                    self.require_enabled(0xE102, "export a scene file")?;
                    let scene = index("scene", 16)? as u32;
                    if let Some(PropInfo {
                        form: Form::Enum { values, settable },
                        ..
                    }) = self.props.get(&0xE0F9)
                    {
                        let listed = if values.is_empty() { settable } else { values };
                        if !listed.is_empty() && !listed.contains(&PtpValue::Int(scene as i128)) {
                            return Err(invalid(format!(
                                "scene {scene} is not among those the camera can export"
                            )));
                        }
                    }
                    if self.download.is_some() {
                        return Err(refused("busy", "another download is in progress"));
                    }
                    let d = Download {
                        id,
                        path: param_str(params, "path")?.to_string(),
                        source: Source::Dataset,
                        offset: 0,
                        total: None,
                        name: Some(format!("scene{scene}")),
                        error: None,
                        closing: false,
                    };
                    let op = Op::new(
                        OP_DOWNLOAD_DATA,
                        vec![DATA_SCENE, scene, 0, 0, 0],
                        Step::DownloadDataset,
                    )
                    .for_command(id);
                    Ok(Plan::Download(d, vec![op]))
                }
                "delete_content" => {
                    if self.deletion.is_some() {
                        return Err(refused("busy", "another deletion is in progress"));
                    }
                    let text = param_str(params, "id")?;
                    let parts: Vec<u32> = text
                        .strip_prefix("c:")
                        .map(|r| r.split(':').filter_map(|p| p.parse().ok()).collect())
                        .unwrap_or_default();
                    let (slot, content) = match parts[..] {
                        [slot, content] | [slot, content, _] => (slot, content),
                        _ => {
                            return Err(invalid(format!(
                                "'{text}' is not a content id from list_content (c:slot:content)"
                            )))
                        }
                    };
                    if self.function != Function::RemoteWithTransfer {
                        return Err(refused(
                            "session_mode",
                            "deleting content needs remote_with_transfer mode",
                        ));
                    }
                    let enable = match slot {
                        1 => 0xE0F3,
                        2 => 0xE0F4,
                        _ => return Err(invalid("'slot' in the id must be 1 or 2")),
                    };
                    self.require_enabled(enable, &format!("delete content in slot {slot}"))?;
                    let op = Op::new(OP_DELETE_CONTENT, vec![content, slot], Step::Delete)
                        .for_command(id);
                    Ok(Plan::Delete(
                        Deletion {
                            id,
                            content,
                            slot,
                            awaiting: false,
                            result: None,
                        },
                        op,
                    ))
                }
                _ => Err(CommandError::UnknownCommand {
                    command: name.into(),
                }),
            }
        })())
        .filter(|r| !matches!(r, Err(CommandError::UnknownCommand { .. })))
    }

    /// Starts an upload: the host file is read first.
    pub(super) fn begin_upload(&mut self, cx: &mut Cx, upload: Upload, path: String, max: u64) {
        self.upload = Some(upload);
        cx.file_read(READ, path, max);
    }

    /// The host file of an upload has been read: check it and send it.
    pub(super) fn upload_read(&mut self, cx: &mut Cx, data: Vec<u8>) {
        let Some(upload) = self.upload.as_ref() else {
            return;
        };
        let id = upload.id;
        match self.upload_ops(upload, &data) {
            Ok(ops) => {
                for op in ops {
                    self.commands.push_back(op.for_command(id));
                }
                self.pump(cx);
            }
            Err(e) => {
                self.upload = None;
                cx.complete(id, Err(e));
            }
        }
    }

    fn upload_ops(&self, upload: &Upload, data: &[u8]) -> Result<Vec<Op>, CommandError> {
        if data.is_empty() {
            return Err(invalid("the file is empty"));
        }
        let awaits = |mut op: Op| {
            op.step = Step::Upload { awaits: true };
            op
        };
        let step = |mut op: Op| {
            op.step = Step::Upload { awaits: false };
            op
        };
        let dataset_version = self
            .props
            .get(&0xD057)
            .and_then(|p| p.current.as_int())
            .unwrap_or(100) as u32;
        let fits = |len: usize| -> Result<(), CommandError> {
            match self.largest_dataset() {
                Some(max) if len as u64 > max => Err(invalid(format!(
                    "the upload is {len} bytes; the camera takes at most {max} in one operation"
                ))),
                _ => Ok(()),
            }
        };
        Ok(match &upload.kind {
            UploadKind::Lut { index } => {
                if !upload.name.to_ascii_lowercase().ends_with(".cube") {
                    return Err(invalid("a LUT is imported from a .cube file"));
                }
                let text = String::from_utf8_lossy(data);
                if !text.contains("LUT_3D_SIZE") && !text.contains("LUT_1D_SIZE") {
                    return Err(invalid(
                        "the file is not a cube LUT (no LUT_3D_SIZE or LUT_1D_SIZE line)",
                    ));
                }
                let version = self.command_version(0xD059, "import a LUT", true)?;
                let dataset = upload_dataset(dataset_version, &upload.name, data);
                fits(dataset.len())?;
                let mut control = version.to_le_bytes().to_vec();
                control.extend_from_slice(&index.to_le_bytes());
                vec![
                    step(Op::with_data(OP_UPLOAD_DATA, vec![DATA_CUBE], dataset)),
                    awaits(Op::with_data(
                        OP_CONTROL_UPLOAD_DATA,
                        vec![CONTROL_BASE_LOOK],
                        control,
                    )),
                ]
            }
            UploadKind::Scene { index } => {
                let version = self.command_version(0xE0E6, "import a scene file", false)?;
                let dataset = upload_dataset(dataset_version, &upload.name, data);
                fits(dataset.len())?;
                let mut control = version.to_le_bytes().to_vec();
                control.extend_from_slice(&index.to_le_bytes());
                vec![
                    step(Op::with_data(OP_UPLOAD_DATA, vec![DATA_SCENE], dataset)),
                    awaits(Op::with_data(
                        OP_CONTROL_UPLOAD_DATA,
                        vec![CONTROL_SCENE],
                        control,
                    )),
                ]
            }
            UploadKind::GridLine { index } => {
                let version = self.command_version(0xE110, "import a grid line file", false)?;
                if version <= 100 && data.len() as u64 > MAX_GRID_LINE {
                    return Err(invalid("a custom grid line file is at most 1 MB"));
                }
                // Each part, with its 12-byte header, within the camera's
                // largest operation.
                let part = self
                    .largest_dataset()
                    .map(|m| (m as usize).saturating_sub(12))
                    .filter(|p| *p > 0)
                    .unwrap_or(1024 * 1024)
                    .min(1024 * 1024);
                let chunks: Vec<&[u8]> = data.chunks(part).collect();
                let count = chunks.len();
                let mut ops: Vec<Op> = chunks
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let offset = (i * part) as u64;
                        step(Op::with_data(
                            OP_UPLOAD_PARTIAL_DATA,
                            vec![
                                DATA_GRID_LINE,
                                offset as u32,
                                (offset >> 32) as u32,
                                c.len() as u32,
                                (i + 1 == count) as u32,
                            ],
                            partial_dataset(c),
                        ))
                    })
                    .collect();
                let name = nul_terminated(&upload.name);
                let mut control = version.to_le_bytes().to_vec();
                control.push(*index);
                control.extend_from_slice(&[0, 0, 0]);
                control.extend_from_slice(&(name.len() as u32).to_le_bytes());
                control.extend_from_slice(&name);
                ops.push(awaits(Op::with_data(
                    OP_CONTROL_UPLOAD_DATA,
                    vec![CONTROL_GRID_LINE],
                    control,
                )));
                ops
            }
            UploadKind::CameraSettings => vec![awaits(Op::with_data(
                OP_SEND_OBJECT,
                vec![CAMERA_SETTING_HANDLE],
                data.to_vec(),
            ))],
            UploadKind::FtpSettings { password } => vec![
                step(Self::password_op(password, upload.id)),
                awaits(Op::with_data(
                    OP_SEND_OBJECT,
                    vec![FTP_SETTING_HANDLE],
                    data.to_vec(),
                )),
            ],
            UploadKind::General { control } => {
                if !String::from_utf8_lossy(&data[..data.len().min(256)]).contains("<?xml") {
                    return Err(invalid("a general setting file is an XML document"));
                }
                let mut wrapped = 8u32.to_le_bytes().to_vec();
                wrapped.extend_from_slice(&(data.len() as u32).to_le_bytes());
                wrapped.extend_from_slice(data);
                vec![awaits(Op::with_data(
                    OP_CONTROL_GENERAL_SETTING_FILE,
                    vec![*control],
                    wrapped,
                ))]
            }
        })
    }

    /// A step of an upload was answered.
    pub(super) fn upload_step(&mut self, cx: &mut Cx, op: &Op, code: u16, awaits: bool) {
        let Some(upload) = self.upload.as_mut() else {
            return;
        };
        if code != RC_OK {
            let id = upload.id;
            self.upload = None;
            self.commands.retain(|o| o.command != Some(id));
            cx.complete(id, Err(rejected(code)));
            return;
        }
        let _ = op;
        if awaits {
            upload.awaiting = true;
            match upload.result.take() {
                Some(result) => self.upload_result(cx, result),
                None => cx.set_timer(UPLOAD_WAIT, RESULT_TIMEOUT),
            }
        }
    }

    /// The camera's result event for an upload.
    pub(super) fn upload_result(&mut self, cx: &mut Cx, result: u32) {
        let Some(upload) = self.upload.as_mut() else {
            return;
        };
        if !upload.awaiting {
            upload.result = Some(result);
            return;
        }
        cx.cancel_timer(UPLOAD_WAIT);
        let words = upload.kind.result_name(result);
        cx.state(json!({"files": {"last_result": {
            "operation": upload.kind.name(),
            "result": words,
        }}}));
        if let UploadKind::General { control } = upload.kind {
            // The camera explains the result in its result file.
            upload.result = Some(result);
            let op = Op::new(
                OP_GET_CONTROL_GENERAL_SETTING_RESULT_FILE,
                vec![control],
                Step::UploadResultFile,
            )
            .for_command(upload.id);
            self.commands.push_front(op);
            self.pump(cx);
            return;
        }
        let upload = self.upload.take().unwrap();
        let outcome = if result == 1 {
            Ok(Outcome::Value {
                value: json!({"result": "ok", "name": upload.name}),
            })
        } else {
            Err(refused(
                "upload_failed",
                format!("the camera reports {words}"),
            ))
        };
        cx.complete(upload.id, outcome);
    }

    /// The general-setting result file arrived.
    pub(super) fn upload_result_file(&mut self, cx: &mut Cx, code: u16, data: &[u8]) {
        let Some(upload) = self.upload.take() else {
            return;
        };
        let words = upload.kind.result_name(upload.result.unwrap_or(0));
        let file = if code == RC_OK {
            embedded_file(data, 2)
                .map(|f| String::from_utf8_lossy(f).into_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };
        cx.complete(
            upload.id,
            Ok(Outcome::Value {
                value: json!({"result": words, "result_file": file}),
            }),
        );
    }

    pub(super) fn upload_timeout(&mut self, cx: &mut Cx) {
        if let Some(upload) = self.upload.take() {
            cx.complete(upload.id, Err(CommandError::Timeout));
        }
    }

    /// A step that must succeed before the rest (the FTP-setting password).
    pub(super) fn prelude(&mut self, cx: &mut Cx, op: &Op, code: u16) {
        if code == RC_OK {
            return;
        }
        let Some(id) = op.command else { return };
        self.commands.retain(|o| o.command != Some(id));
        if self.upload.as_ref().is_some_and(|u| u.id == id) {
            self.upload = None;
        }
        if self.download.as_ref().is_some_and(|d| d.id == id) {
            self.download = None;
        }
        cx.complete(id, Err(rejected(code)));
    }

    /// A scene file export arrived: write the file inside it.
    pub(super) fn download_dataset(&mut self, cx: &mut Cx, code: u16, data: &[u8]) {
        let Some(d) = self.download.as_ref() else {
            return;
        };
        let file = if code == RC_OK {
            embedded_file(data, 3).map(<[u8]>::to_vec)
        } else {
            Err(props::response_name(code))
        };
        match file {
            Ok(file) => {
                cx.file_open(FILE, d.path.clone());
                cx.file_write(FILE, file);
                self.close_download(cx);
            }
            Err(e) => {
                let d = self.download.take().unwrap();
                cx.complete(d.id, Err(refused("download_failed", e)));
            }
        }
    }

    /// The camera answered a deletion request; its result event follows.
    pub(super) fn delete_step(&mut self, cx: &mut Cx, code: u16) {
        self.last_delete_at = Some(cx.now());
        let Some(deletion) = self.deletion.as_mut() else {
            return;
        };
        if code != RC_OK {
            let id = deletion.id;
            self.deletion = None;
            cx.complete(id, Err(rejected(code)));
            return;
        }
        deletion.awaiting = true;
        let content = deletion.content;
        match deletion.result.take() {
            Some(result) => self.delete_result(cx, result, content),
            None => cx.set_timer(DELETE_WAIT, DELETE_TIMEOUT),
        }
    }

    pub(super) fn delete_result(&mut self, cx: &mut Cx, result: u32, content: u32) {
        let Some(deletion) = self.deletion.as_mut() else {
            return;
        };
        if deletion.content != content {
            return;
        }
        if !deletion.awaiting {
            deletion.result = Some(result);
            return;
        }
        cx.cancel_timer(DELETE_WAIT);
        let deletion = self.deletion.take().unwrap();
        let prefix = format!("c:{}:{}:", deletion.slot, deletion.content);
        self.sizes.retain(|k, _| !k.starts_with(&prefix));
        let words = delete_result_name(result);
        cx.complete(
            deletion.id,
            if result == 1 {
                Ok(Outcome::Ack)
            } else {
                Err(refused(
                    "delete_failed",
                    format!("the camera reports {words}"),
                ))
            },
        );
    }

    pub(super) fn delete_timeout(&mut self, cx: &mut Cx) {
        if let Some(d) = self.deletion.take() {
            cx.complete(d.id, Err(CommandError::Timeout));
        }
    }

    /// Spacing before a deletion, as the reference asks.
    pub(super) fn delete_spacing(&self, now: Millis) -> Option<Millis> {
        let last = self.last_delete_at?;
        let since = now.saturating_sub(last);
        (since < DELETE_SPACING).then(|| DELETE_SPACING - since)
    }

    /// The camera's events for these transfers.
    pub(super) fn file_event(&mut self, cx: &mut Cx, code: u16, p: &dyn Fn(usize) -> u32) {
        match code {
            0xC214 => {
                let matches = self
                    .upload
                    .as_ref()
                    .and_then(|u| u.kind.control_type())
                    .is_some_and(|t| t == p(1));
                if matches {
                    self.upload_result(cx, p(0));
                }
            }
            0xC209 => {
                if self
                    .upload
                    .as_ref()
                    .is_some_and(|u| u.kind == UploadKind::CameraSettings)
                {
                    self.upload_result(cx, p(0));
                }
            }
            0xC20A => {
                if self
                    .upload
                    .as_ref()
                    .is_some_and(|u| matches!(u.kind, UploadKind::FtpSettings { .. }))
                {
                    self.upload_result(cx, p(0));
                }
            }
            0xC21A => {
                let matches = self
                    .upload
                    .as_ref()
                    .is_some_and(|u| u.kind == UploadKind::General { control: p(1) });
                if matches {
                    self.upload_result(cx, p(0));
                }
            }
            0xC240 => self.delete_result(cx, p(0), p(1)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_datasets() {
        let b = upload_dataset(100, "a.cube", b"LUT");
        assert_eq!(&b[..4], &100u32.to_le_bytes());
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(b[8..12].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(b[12..16].try_into().unwrap()), 27);
        assert_eq!(&b[20..27], b"a.cube\0");
        assert_eq!(&b[27..], b"LUT");
        let p = partial_dataset(b"xy");
        assert_eq!(p, [100, 0, 0, 0, 12, 0, 0, 0, 2, 0, 0, 0, b'x', b'y']);
    }

    #[test]
    fn embedded_files() {
        let mut d = 100u32.to_le_bytes().to_vec();
        d.extend_from_slice(&16u32.to_le_bytes());
        d.extend_from_slice(&3u32.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
        d.extend_from_slice(b"abc");
        assert_eq!(embedded_file(&d, 3).unwrap(), b"abc");
        assert!(embedded_file(&d[..17], 3).is_err());
        assert_eq!(file_name("C:\\luts\\Show.cube"), "Show.cube");
        assert_eq!(file_name("/tmp/x.cube"), "x.cube");
    }
}
