// Anka Browser - Çok Süreçli (Multi-Process) Sekme İzolasyonu ve Sıfır-Crate IPC (v0.3.2)
// Her tarayıcı sekmesi, tarayıcı ana penceresinden (`Browser Process`) ayrı bir
// `--renderer-worker` işletim sistemi alt sürecinde (`Renderer Process`) çalışır.
// Bir sekme çökerse diğer sekmeler ve tarayıcı penceresi hayatta kalır!

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use anka_paint::{DecodedImage, PaintCommand, RgbColor};

use crate::{LoadedPage, html_motoru_calistir_kalkanli, motoru_calistir_istekli};

/// Bir sekmenin izole işçi sürecinden (`--renderer-worker`) ana arayüz sürecine
/// gönderdiği eksiksiz çizim anlık görüntüsü (Snapshot):
#[derive(Clone)]
pub struct RenderSnapshot {
    pub pid: u32,
    pub crashed: bool,
    pub page_title: Option<String>,
    pub page_bg: Option<RgbColor>,
    pub status_message: String,
    pub display_list: Vec<PaintCommand>,
    pub status_code: u16,
    pub body_len: usize,
    pub shield_blocked: usize,
    pub set_cookies: Vec<String>,
    pub js_logs: Vec<String>,
}

impl RenderSnapshot {
    pub fn empty() -> Self {
        Self {
            pid: 0,
            crashed: false,
            page_title: None,
            page_bg: None,
            status_message: String::new(),
            display_list: Vec::new(),
            status_code: 0,
            body_len: 0,
            shield_blocked: 0,
            set_cookies: Vec::new(),
            js_logs: Vec::new(),
        }
    }

    pub fn from_loaded_page(page: &LoadedPage, pid: u32) -> Self {
        Self {
            pid,
            crashed: false,
            page_title: page.page_title.clone(),
            page_bg: page.page_bg,
            status_message: page.status_message.clone(),
            display_list: page.display_list.clone(),
            status_code: page.status_code,
            body_len: page.body_len,
            shield_blocked: page.shield_stats.total_blocked(),
            set_cookies: page.set_cookies.clone(),
            js_logs: page.js_runtime.logs.clone(),
        }
    }

    /// Bir sekme işçi süreci (`PID`) beklenmedik şekilde çöktüğünde, tarayıcının
    /// diğer sekmelerini korumak için üretilen güvenli kurtarma sayfası:
    pub fn crashed_page(pid: u32, reason: &str) -> Self {
        let html = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
             <html><head><title>Sekme Coktu — Anka Kurtarma</title>\
             <style>\
             .kurtarma {{ color: #991b1b; background-color: #fef2f2; border: 2px solid #ef4444; padding: 14px; }}\
             h1 {{ color: #b91c1c; font-size: 24px; }}\
             p {{ color: #334155; font-size: 15px; }}\
             .pill {{ color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; font-size: 15px; }}\
             </style></head><body>\
             <div class=\"kurtarma\">\
             <h1>Bu Sekmenin Islem Sureci Coktu (PID: {pid})</h1>\
             <p>Cok surecli sekme izolasyonu sayesinde tarayici pencereniz ve diger acik sekmeleriniz hic etkilenmeden calismaya devam ediyor.</p>\
             <p>Detay: {reason}</p>\
             <a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Yeni Surecle Ana Sayfaya Don</a>\
             </div></body></html>"
        );
        let page = html_motoru_calistir_kalkanli("http://anka.local/cokme-kurtarma", &html, 860, true);
        let mut snap = Self::from_loaded_page(&page, pid);
        snap.crashed = true;
        snap.status_message = format!("Sekme Sureci Coktu (PID: {pid}) — Kurtarma Hazir");
        snap
    }
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    let bytes = hex.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let val = |b: u8| -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let (pairs, _) = bytes.as_chunks::<2>();
    for &[c_hi, c_lo] in pairs {
        let hi = val(c_hi)?;
        let lo = val(c_lo)?;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

pub fn decode_hex_string(hex: &str) -> Option<String> {
    let raw = decode_hex(hex)?;
    Some(String::from_utf8_lossy(&raw).into_owned())
}

fn encode_opt_str_hex(opt: Option<&str>) -> String {
    match opt {
        Some(s) => format!("1:{}", encode_hex(s.as_bytes())),
        None => "0".to_string(),
    }
}

fn decode_opt_str_hex(raw: &str) -> Option<String> {
    if raw == "0" {
        return None;
    }
    let hex = raw.strip_prefix("1:")?;
    decode_hex_string(hex)
}

fn encode_rgb(c: RgbColor) -> String {
    format!("{},{},{}", c.r, c.g, c.b)
}

fn decode_rgb(s: &str) -> Option<RgbColor> {
    let mut it = s.split(',');
    let r = it.next()?.parse::<u8>().ok()?;
    let g = it.next()?.parse::<u8>().ok()?;
    let b = it.next()?.parse::<u8>().ok()?;
    Some(RgbColor { r, g, b })
}

fn encode_opt_rgb(opt: Option<RgbColor>) -> String {
    match opt {
        Some(c) => encode_rgb(c),
        None => "-".to_string(),
    }
}

fn decode_opt_rgb(s: &str) -> Option<RgbColor> {
    if s == "-" { None } else { decode_rgb(s) }
}

/// Bir `RenderSnapshot` paketini pipe (`stdout -> stdin`) üzerinden tek parça halinde gönderilebilir metne çevirir.
pub fn serialize_snapshot(snap: &RenderSnapshot) -> String {
    let mut out = String::new();
    let title_enc = encode_opt_str_hex(snap.page_title.as_deref());
    let status_enc = encode_hex(snap.status_message.as_bytes());
    out.push_str(&format!(
        "META\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        snap.pid,
        u8::from(snap.crashed),
        snap.status_code,
        snap.body_len,
        snap.shield_blocked,
        encode_opt_rgb(snap.page_bg),
        title_enc,
        status_enc,
        snap.display_list.len(),
        snap.set_cookies.len(),
        snap.js_logs.len()
    ));

    for cmd in &snap.display_list {
        let (img_meta, img_hex) = if let Some(img) = &cmd.image {
            (
                format!("1,{},{}", img.width, img.height),
                encode_hex(&img.rgba),
            )
        } else {
            ("0".to_string(), String::new())
        };
        out.push_str(&format!(
            "CMD\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            cmd.x,
            cmd.y,
            cmd.width,
            cmd.height,
            cmd.font_size,
            cmd.padding,
            encode_hex(cmd.text_align.as_bytes()),
            encode_rgb(cmd.color),
            encode_opt_rgb(cmd.bg_color),
            encode_opt_rgb(cmd.border_color),
            img_meta,
            img_hex,
            encode_hex(cmd.tag.as_bytes()),
            encode_hex(cmd.text.as_bytes()),
            encode_opt_str_hex(cmd.href.as_deref()),
            encode_opt_str_hex(cmd.image_src.as_deref()),
            encode_opt_str_hex(cmd.onclick.as_deref()),
            encode_opt_str_hex(cmd.input_name.as_deref()),
            encode_opt_str_hex(cmd.form_action.as_deref()),
            u8::from(cmd.is_submit),
            u8::from(cmd.is_password),
            u8::from(cmd.is_post_form),
            cmd.border_radius,
            u8::from(cmd.is_multiline),
            u8::from(cmd.has_shadow),
            u8::from(cmd.is_bold),
            u8::from(cmd.is_italic)
        ));
    }
    for cookie in &snap.set_cookies {
        out.push_str(&format!("COOKIE\t{}\n", encode_hex(cookie.as_bytes())));
    }
    for log_line in &snap.js_logs {
        out.push_str(&format!("LOG\t{}\n", encode_hex(log_line.as_bytes())));
    }
    out.push_str("END\n");
    out
}

/// Pipe üzerinden gelen IPC satırlarını çözüp `RenderSnapshot` oluşturur.
/// Worker beklenmedik şekilde kapandıysa (`EOF`) hata döndürerek çökme kurtarmasını tetikler!
pub fn deserialize_snapshot<R: BufRead>(reader: &mut R) -> Result<RenderSnapshot, String> {
    let mut header = String::new();
    let n = reader
        .read_line(&mut header)
        .map_err(|e| format!("IPC okuma hatasi: {e}"))?;
    if n == 0 {
        return Err("Alt surec yanit vermeden kapandi (EOF)".to_string());
    }

    let meta_parts: Vec<&str> = header.trim_end_matches(['\r', '\n']).split('\t').collect();
    if meta_parts.len() != 12 || meta_parts[0] != "META" {
        return Err("Gecersiz IPC META basligi".to_string());
    }

    let pid = meta_parts[1].parse::<u32>().unwrap_or(0);
    let crashed = meta_parts[2] == "1";
    let status_code = meta_parts[3].parse::<u16>().unwrap_or(0);
    let body_len = meta_parts[4].parse::<usize>().unwrap_or(0);
    let shield_blocked = meta_parts[5].parse::<usize>().unwrap_or(0);
    let page_bg = decode_opt_rgb(meta_parts[6]);
    let page_title = decode_opt_str_hex(meta_parts[7]);
    let status_message = decode_hex_string(meta_parts[8]).unwrap_or_default();
    let cmd_count = meta_parts[9].parse::<usize>().unwrap_or(0);
    let cookie_count = meta_parts[10].parse::<usize>().unwrap_or(0);
    let log_count = meta_parts[11].parse::<usize>().unwrap_or(0);

    let mut display_list = Vec::with_capacity(cmd_count);
    for _ in 0..cmd_count {
        let mut line = String::new();
        let read_n = reader
            .read_line(&mut line)
            .map_err(|e| format!("IPC CMD okuma hatasi: {e}"))?;
        if read_n == 0 {
            return Err("Alt surec veri aktarimi sirasinda sonlandi".to_string());
        }
        let p: Vec<&str> = line.trim_end_matches(['\r', '\n']).split('\t').collect();
        if p.len() != 28 || p[0] != "CMD" {
            return Err("Bozuk IPC CMD satiri".to_string());
        }

        let image = if let Some(dims) = p[11].strip_prefix("1,") {
            let mut it = dims.split(',');
            let w = it.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            let h = it.next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            let rgba = decode_hex(p[12]).unwrap_or_default();
            if w > 0 && h > 0 && rgba.len() == (w * h * 4) as usize {
                Some(DecodedImage {
                    width: w,
                    height: h,
                    rgba,
                })
            } else {
                None
            }
        } else {
            None
        };

        display_list.push(PaintCommand {
            tag: decode_hex_string(p[13]).unwrap_or_default(),
            text: decode_hex_string(p[14]).unwrap_or_default(),
            x: p[1].parse().unwrap_or(0),
            y: p[2].parse().unwrap_or(0),
            width: p[3].parse().unwrap_or(0),
            height: p[4].parse().unwrap_or(0),
            font_size: p[5].parse().unwrap_or(16),
            padding: p[6].parse().unwrap_or(0),
            text_align: decode_hex_string(p[7]).unwrap_or_else(|| "left".to_string()),
            color: decode_rgb(p[8]).unwrap_or(RgbColor { r: 0, g: 0, b: 0 }),
            bg_color: decode_opt_rgb(p[9]),
            border_color: decode_opt_rgb(p[10]),
            href: decode_opt_str_hex(p[15]),
            image_src: decode_opt_str_hex(p[16]),
            image,
            onclick: decode_opt_str_hex(p[17]),
            input_name: decode_opt_str_hex(p[18]),
            form_action: decode_opt_str_hex(p[19]),
            is_submit: p[20] == "1",
            is_password: p[21] == "1",
            is_post_form: p[22] == "1",
            border_radius: p[23].parse().unwrap_or(0),
            is_multiline: p[24] == "1",
            has_shadow: p[25] == "1",
            is_bold: p[26] == "1",
            is_italic: p[27] == "1",
        });
    }

    let mut set_cookies = Vec::with_capacity(cookie_count);
    for _ in 0..cookie_count {
        let mut line = String::new();
        let read_n = reader
            .read_line(&mut line)
            .map_err(|e| format!("IPC COOKIE okuma hatasi: {e}"))?;
        if read_n == 0 {
            return Err("Alt surec cerez aktarimi sirasinda sonlandi".to_string());
        }
        let hex = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix("COOKIE\t")
            .ok_or_else(|| "Bozuk IPC COOKIE satiri".to_string())?;
        if let Some(cookie_line) = decode_hex_string(hex) {
            set_cookies.push(cookie_line);
        }
    }

    let mut js_logs = Vec::with_capacity(log_count);
    for _ in 0..log_count {
        let mut line = String::new();
        let read_n = reader
            .read_line(&mut line)
            .map_err(|e| format!("IPC LOG okuma hatasi: {e}"))?;
        if read_n == 0 {
            return Err("Alt surec log aktarimi sirasinda sonlandi".to_string());
        }
        let hex = line
            .trim_end_matches(['\r', '\n'])
            .strip_prefix("LOG\t")
            .ok_or_else(|| "Bozuk IPC LOG satiri".to_string())?;
        if let Some(log_msg) = decode_hex_string(hex) {
            js_logs.push(log_msg);
        }
    }

    let mut end_line = String::new();
    let end_n = reader
        .read_line(&mut end_line)
        .map_err(|e| format!("IPC END okuma hatasi: {e}"))?;
    if end_n == 0 || end_line.trim() != "END" {
        return Err("IPC END isaretcisi eksik".to_string());
    }

    Ok(RenderSnapshot {
        pid,
        crashed,
        page_title,
        page_bg,
        status_message,
        display_list,
        status_code,
        body_len,
        shield_blocked,
        set_cookies,
        js_logs,
    })
}

/// İzole işçi sürecinin içinde yaşayan sekme motoru durumu:
pub struct WorkerState {
    current_page: Option<LoadedPage>,
}

impl WorkerState {
    pub fn new() -> Self {
        Self { current_page: None }
    }

    pub fn process_request_line(&mut self, line: &str) -> Option<RenderSnapshot> {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let parts: Vec<&str> = trimmed.split('\t').collect();
        let pid = std::process::id();

        match parts.as_slice() {
            ["LOAD", vw_s, sh_s, addr_hex] => {
                let vw = vw_s.parse::<u32>().unwrap_or(860);
                let shield_on = *sh_s != "0";
                let addr = decode_hex_string(addr_hex).unwrap_or_default();
                let page = motoru_calistir_istekli(&addr, "GET", None, None, vw, shield_on);
                let snap = RenderSnapshot::from_loaded_page(&page, pid);
                self.current_page = Some(page);
                Some(snap)
            }
            ["LOAD", vw_s, sh_s, addr_hex, cookie_hex] => {
                let vw = vw_s.parse::<u32>().unwrap_or(860);
                let shield_on = *sh_s != "0";
                let addr = decode_hex_string(addr_hex).unwrap_or_default();
                let cookie_opt = decode_opt_str_hex(cookie_hex);
                let page = motoru_calistir_istekli(
                    &addr,
                    "GET",
                    None,
                    cookie_opt.as_deref(),
                    vw,
                    shield_on,
                );
                let snap = RenderSnapshot::from_loaded_page(&page, pid);
                self.current_page = Some(page);
                Some(snap)
            }
            ["POST", vw_s, sh_s, addr_hex, body_hex, cookie_hex] => {
                let vw = vw_s.parse::<u32>().unwrap_or(860);
                let shield_on = *sh_s != "0";
                let addr = decode_hex_string(addr_hex).unwrap_or_default();
                let body = decode_hex_string(body_hex).unwrap_or_default();
                let cookie_opt = decode_opt_str_hex(cookie_hex);
                let page = motoru_calistir_istekli(
                    &addr,
                    "POST",
                    Some(&body),
                    cookie_opt.as_deref(),
                    vw,
                    shield_on,
                );
                let snap = RenderSnapshot::from_loaded_page(&page, pid);
                self.current_page = Some(page);
                Some(snap)
            }
            ["LOAD_HTML", vw_s, sh_s, url_hex, html_hex] => {
                let vw = vw_s.parse::<u32>().unwrap_or(860);
                let shield_on = *sh_s != "0";
                let url_s = decode_hex_string(url_hex).unwrap_or_default();
                let raw_html = decode_hex_string(html_hex).unwrap_or_default();
                let page = html_motoru_calistir_kalkanli(&url_s, &raw_html, vw, shield_on);
                let snap = RenderSnapshot::from_loaded_page(&page, pid);
                self.current_page = Some(page);
                Some(snap)
            }
            ["EVAL_JS", vw_s, js_hex] => {
                let vw = vw_s.parse::<u32>().unwrap_or(860);
                let js_code = decode_hex_string(js_hex).unwrap_or_default();
                let page = self.current_page.as_mut()?;
                page.run_inline_js(&js_code, vw);
                Some(RenderSnapshot::from_loaded_page(page, pid))
            }
            _ => None,
        }
    }
}

/// `--renderer-worker` olarak başlatılan izole OS alt sürecinin ana IPC döngüsü:
pub fn run_renderer_worker_stdio() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut state = WorkerState::new();

    for line_res in stdin.lock().lines() {
        let Ok(line) = line_res else {
            break;
        };
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "QUIT" {
            break;
        }
        if trimmed == "CRASH" {
            std::process::exit(101);
        }
        if let Some(rest) = trimmed.strip_prefix("LOAD\t")
            && let Some(addr_hex) = rest.split('\t').nth(2)
            && let Some(addr) = decode_hex_string(addr_hex)
            && addr.contains("anka.local/cokme-testi")
        {
            // Çok süreçli izolasyonu doğrulamak için yalnızca bu sekme sürecini sonlandır!
            std::process::exit(101);
        }

        let snap = state.process_request_line(trimmed).unwrap_or_else(|| {
            RenderSnapshot::from_loaded_page(
                &LoadedPage::error("IPC Worker: Gecersiz istek".to_string()),
                std::process::id(),
            )
        });
        let wire = serialize_snapshot(&snap);
        if out.write_all(wire.as_bytes()).is_err() || out.flush().is_err() {
            break;
        }
    }
}

enum WorkerBackend {
    Subprocess {
        child: Child,
        stdin: BufWriter<ChildStdin>,
        stdout: BufReader<ChildStdout>,
        pid: u32,
        dead: bool,
    },
    InProcess {
        worker: Box<WorkerState>,
        crashed: bool,
        virtual_pid: u32,
    },
}

/// Tarayıcı UI sürecinin her bir sekme için yönettiği izole işçi istemcisi:
pub struct TabWorkerClient {
    backend: WorkerBackend,
    allow_subprocess: bool,
}

impl TabWorkerClient {
    pub fn new(allow_subprocess: bool) -> Self {
        let backend = Self::create_backend(allow_subprocess);
        Self {
            backend,
            allow_subprocess,
        }
    }

    fn try_spawn_subprocess() -> Option<WorkerBackend> {
        let exe = std::env::current_exe().ok()?;
        let mut child = Command::new(exe)
            .arg("--renderer-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let pid = child.id();
        Some(WorkerBackend::Subprocess {
            child,
            stdin: BufWriter::new(stdin),
            stdout: BufReader::new(stdout),
            pid,
            dead: false,
        })
    }

    fn create_backend(allow_subprocess: bool) -> WorkerBackend {
        if allow_subprocess && let Some(sub) = Self::try_spawn_subprocess() {
            return sub;
        }
        WorkerBackend::InProcess {
            worker: Box::new(WorkerState::new()),
            crashed: false,
            virtual_pid: std::process::id(),
        }
    }

    fn ensure_alive(&mut self) {
        let need_respawn = match &mut self.backend {
            WorkerBackend::Subprocess { child, dead, .. } => {
                *dead || !matches!(child.try_wait(), Ok(None))
            }
            WorkerBackend::InProcess { crashed, .. } => *crashed,
        };
        if need_respawn {
            self.shutdown_backend();
            self.backend = Self::create_backend(self.allow_subprocess);
        }
    }

    fn shutdown_backend(&mut self) {
        if let WorkerBackend::Subprocess {
            child,
            stdin,
            dead,
            ..
        } = &mut self.backend
        {
            if !*dead {
                let _ = stdin.write_all(b"QUIT\n");
                let _ = stdin.flush();
                if matches!(child.try_wait(), Ok(None)) {
                    let _ = child.kill();
                }
            }
            let _ = child.wait();
            *dead = true;
        }
    }

    fn send_command_line(&mut self, line: &str, respawn_if_dead: bool) -> RenderSnapshot {
        if respawn_if_dead {
            self.ensure_alive();
        }

        match &mut self.backend {
            WorkerBackend::Subprocess {
                child,
                stdin,
                stdout,
                pid,
                dead,
            } => {
                let worker_pid = *pid;
                if *dead || stdin.write_all(line.as_bytes()).is_err() || stdin.flush().is_err() {
                    *dead = true;
                    let _ = child.wait();
                    return RenderSnapshot::crashed_page(
                        worker_pid,
                        "Islem kanali kapandi (Broken pipe)",
                    );
                }
                match deserialize_snapshot(stdout) {
                    Ok(snap) => snap,
                    Err(reason) => {
                        *dead = true;
                        let exit_info = match child.wait() {
                            Ok(st) => format!("{reason} ({st})"),
                            Err(_) => reason,
                        };
                        RenderSnapshot::crashed_page(worker_pid, &exit_info)
                    }
                }
            }
            WorkerBackend::InProcess {
                worker,
                crashed,
                virtual_pid,
            } => {
                let trimmed = line.trim_end_matches(['\r', '\n']);
                if trimmed == "CRASH"
                    || trimmed
                        .strip_prefix("LOAD\t")
                        .and_then(|rest| rest.split('\t').nth(2))
                        .and_then(decode_hex_string)
                        .is_some_and(|a| a.contains("anka.local/cokme-testi"))
                {
                    *crashed = true;
                    return RenderSnapshot::crashed_page(
                        *virtual_pid,
                        "Simule edilmis isci surec cokmesi (exit status: 101)",
                    );
                }

                worker.process_request_line(trimmed).unwrap_or_else(|| {
                    RenderSnapshot::from_loaded_page(
                        &LoadedPage::error("InProcess Worker hatasi".to_string()),
                        *virtual_pid,
                    )
                })
            }
        }
    }

    pub fn load_url(
        &mut self,
        address: &str,
        viewport_width: u32,
        shield_enabled: bool,
        cookie_header: Option<&str>,
    ) -> RenderSnapshot {
        let cmd = format!(
            "LOAD\t{}\t{}\t{}\t{}\n",
            viewport_width,
            u8::from(shield_enabled),
            encode_hex(address.as_bytes()),
            encode_opt_str_hex(cookie_header)
        );
        self.send_command_line(&cmd, true)
    }

    pub fn post_url(
        &mut self,
        address: &str,
        body: &str,
        viewport_width: u32,
        shield_enabled: bool,
        cookie_header: Option<&str>,
    ) -> RenderSnapshot {
        let cmd = format!(
            "POST\t{}\t{}\t{}\t{}\t{}\n",
            viewport_width,
            u8::from(shield_enabled),
            encode_hex(address.as_bytes()),
            encode_hex(body.as_bytes()),
            encode_opt_str_hex(cookie_header)
        );
        self.send_command_line(&cmd, true)
    }

    pub fn load_html(
        &mut self,
        url: &str,
        raw_http: &str,
        viewport_width: u32,
        shield_enabled: bool,
    ) -> RenderSnapshot {
        let cmd = format!(
            "LOAD_HTML\t{}\t{}\t{}\t{}\n",
            viewport_width,
            u8::from(shield_enabled),
            encode_hex(url.as_bytes()),
            encode_hex(raw_http.as_bytes())
        );
        self.send_command_line(&cmd, true)
    }

    pub fn eval_js(&mut self, js_code: &str, viewport_width: u32) -> RenderSnapshot {
        let cmd = format!(
            "EVAL_JS\t{}\t{}\n",
            viewport_width,
            encode_hex(js_code.as_bytes())
        );
        self.send_command_line(&cmd, false)
    }
}

impl Drop for TabWorkerClient {
    fn drop(&mut self) {
        self.shutdown_backend();
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::motoru_calistir_kalkanli;

    use super::*;

    #[test]
    fn test_ipc_snapshot_serileme_ve_cozme() {
        let page = motoru_calistir_kalkanli("http://anka.local/hosgeldiniz", 860, true);
        let original = RenderSnapshot::from_loaded_page(&page, 4242);

        let wire = serialize_snapshot(&original);
        let mut cursor = Cursor::new(wire.into_bytes());
        let decoded = deserialize_snapshot(&mut cursor).expect("IPC snapshot cozulmeli");

        assert_eq!(decoded.pid, 4242);
        assert_eq!(decoded.page_title, original.page_title);
        assert_eq!(decoded.status_message, original.status_message);
        assert_eq!(decoded.display_list.len(), original.display_list.len());
        assert_eq!(decoded.js_logs, original.js_logs);
        assert!(!decoded.js_logs.is_empty());
        assert!(
            decoded
                .display_list
                .iter()
                .any(|c| c.border_radius == 8 && c.has_shadow)
        );
        assert!(
            decoded
                .display_list
                .iter()
                .any(|c| c.tag == "a" && c.is_bold)
        );
        assert!(
            decoded
                .display_list
                .iter()
                .any(|c| c.is_italic)
        );

        // Gömülü bitmap görsel pikselleri de bozulmadan taşınmış olmalı!
        let orig_img = original
            .display_list
            .iter()
            .find(|c| c.image.is_some())
            .and_then(|c| c.image.as_ref())
            .expect("Anka amblemi bulunmali");
        let dec_img = decoded
            .display_list
            .iter()
            .find(|c| c.image.is_some())
            .and_then(|c| c.image.as_ref())
            .expect("Cozulen amblem bulunmali");
        assert_eq!(dec_img.width, orig_img.width);
        assert_eq!(dec_img.height, orig_img.height);
        assert_eq!(dec_img.rgba, orig_img.rgba);
    }

    #[test]
    fn test_ipc_eof_durumunda_hata_doner() {
        let mut empty = Cursor::new(Vec::<u8>::new());
        let res = deserialize_snapshot(&mut empty);
        assert!(res.is_err());
    }

    #[test]
    fn test_ipc_post_ve_set_cookie_tasima() {
        let mut worker = WorkerState::new();
        let req = format!(
            "POST\t860\t1\t{}\t{}\t0\n",
            encode_hex(b"http://anka.local/oturum"),
            encode_hex(b"kullanici=ekrem&sifre=1234")
        );
        let snap = worker
            .process_request_line(&req)
            .expect("POST istegi snapshot donmeli");
        assert!(
            snap.set_cookies
                .iter()
                .any(|c| c.contains("anka_user=ekrem"))
        );

        let wire = serialize_snapshot(&snap);
        let mut cursor = Cursor::new(wire.into_bytes());
        let decoded = deserialize_snapshot(&mut cursor).expect("POST snapshot cozulmeli");
        assert_eq!(decoded.set_cookies, snap.set_cookies);
        assert!(
            decoded
                .display_list
                .iter()
                .any(|b| b.text.contains("Hos geldiniz, ekrem!"))
        );

        // Parola kutusu ve POST form bayraklari GET /oturum icin de IPC uzerinden korunmali:
        let get_req = format!(
            "LOAD\t860\t1\t{}\t0\n",
            encode_hex(b"http://anka.local/oturum")
        );
        let form_snap = worker.process_request_line(&get_req).unwrap();
        let form_wire = serialize_snapshot(&form_snap);
        let mut form_cursor = Cursor::new(form_wire.into_bytes());
        let form_dec = deserialize_snapshot(&mut form_cursor).unwrap();
        let pw_cmd = form_dec
            .display_list
            .iter()
            .find(|c| c.input_name.as_deref() == Some("sifre"))
            .expect("sifre kutusu olmali");
        assert!(pw_cmd.is_password);
        assert!(pw_cmd.is_post_form);
    }
}
