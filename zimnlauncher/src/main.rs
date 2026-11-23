#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Align, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, vec2};
use serde::{Deserialize, Serialize};
#[cfg(not(windows))]
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const BG: Color32 = Color32::from_rgb(14, 14, 20);
const CARD: Color32 = Color32::from_rgb(25, 25, 35);
const CARD_HOVER: Color32 = Color32::from_rgb(36, 36, 52);
const PALETTE: [Color32; 6] = [
    Color32::from_rgb(90, 60, 190),
    Color32::from_rgb(30, 120, 170),
    Color32::from_rgb(40, 140, 100),
    Color32::from_rgb(180, 80, 80),
    Color32::from_rgb(190, 130, 40),
    Color32::from_rgb(150, 60, 150),
];

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
enum Kind {
    #[default]
    Exe,
    AppImage,
    Jar,
    Apk,
}

impl Kind {
    fn from_path(p: &std::path::Path) -> Self {
        match p
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref()
        {
            Some("appimage") => Kind::AppImage,
            Some("jar") => Kind::Jar,
            Some("apk") => Kind::Apk,
            _ => Kind::Exe,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Exe => "WINDOWS",
            Kind::AppImage => "APPIMAGE",
            Kind::Jar => "JAVA",
            Kind::Apk => "ANDROID",
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct Game {
    name: String,
    exe: PathBuf,
    prefix: Option<PathBuf>,
    icon: Option<PathBuf>,
    #[serde(default)]
    kind: Kind,
    package: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
enum ApkMode {
    #[default]
    Waydroid,
    Adb,
    Custom,
}

enum Note {
    Status(String),
    Package(PathBuf, String),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Settings {
    wine_bin: String,
    java_bin: String,
    apk_mode: ApkMode,
    apk_cmd: String,
    adb_bin: String,
    adb_connect: String,
    emulator_bin: String,
    avd: String,
    default_prefix: Option<PathBuf>,
    icon_size: f32,
    accent: [u8; 3],
    esync: bool,
    fsync: bool,
    quiet: bool,
    dxvk_hud: bool,
    close_on_launch: bool,
    sort: bool,
    transparent: bool,
    opacity: f32,
    bg_color: [u8; 3],
    bg_image: Option<PathBuf>,
    bg_dim: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wine_bin: "wine".into(),
            java_bin: "java".into(),
            apk_mode: if cfg!(windows) { ApkMode::Adb } else { ApkMode::Waydroid },
            apk_cmd: String::new(),
            adb_bin: "adb".into(),
            adb_connect: String::new(),
            emulator_bin: "emulator".into(),
            avd: String::new(),
            default_prefix: None,
            icon_size: 220.0,
            accent: [124, 92, 255],
            esync: false,
            fsync: false,
            quiet: true,
            dxvk_hud: false,
            close_on_launch: false,
            sort: true,
            transparent: false,
            opacity: 0.85,
            bg_color: [14, 14, 20],
            bg_image: None,
            bg_dim: 0.45,
        }
    }
}

#[derive(Clone, Copy)]
enum Target {
    Add,
    Icon(usize),
    Prefix(usize),
    DefPrefix,
    BgImage,
}

type Msg = (Target, Option<PathBuf>);

struct App {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    note_tx: Sender<Note>,
    note_rx: Receiver<Note>,
    busy: bool,
    transparent_at_start: bool,
    games: Vec<Game>,
    settings: Settings,
    search: String,
    show_settings: bool,
    status: String,
}

fn dir() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("zimnlauncher")
}

fn import_icon(src: PathBuf) -> PathBuf {
    let d = dir().join("icons");
    let _ = fs::create_dir_all(&d);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_nanos())
        .unwrap_or(0);
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "png".into());
    let dst = d.join(format!("{ts}.{ext}"));
    if fs::copy(&src, &dst).is_ok() {
        dst
    } else {
        src
    }
}

fn placeholder(ui: &egui::Ui, name: &str, rect: Rect) {
    let h = name
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    ui.painter().rect_filled(rect, 16.0, PALETTE[h % PALETTE.len()]);
    ui.painter()
        .circle_filled(rect.center(), rect.width() * 0.38, Color32::from_white_alpha(22));
    let letter = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    ui.painter().text(
        rect.center() - vec2(0.0, rect.height() * 0.06),
        Align2::CENTER_CENTER,
        letter,
        FontId::proportional(rect.width() * 0.44),
        Color32::WHITE,
    );
}

fn style(ctx: &egui::Context, accent: Color32) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.window_rounding = egui::Rounding::same(16.0);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(45, 45, 60));
    v.selection.bg_fill = accent;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(40, 40, 54));
    let base = Color32::from_rgb(36, 36, 50);
    let hover = Color32::from_rgb(56, 56, 78);
    v.widgets.inactive.bg_fill = base;
    v.widgets.inactive.weak_bg_fill = base;
    v.widgets.hovered.bg_fill = hover;
    v.widgets.hovered.weak_bg_fill = hover;
    v.widgets.active.bg_fill = accent;
    v.widgets.active.weak_bg_fill = accent;
    let r = egui::Rounding::same(10.0);
    v.widgets.noninteractive.rounding = r;
    v.widgets.inactive.rounding = r;
    v.widgets.hovered.rounding = r;
    v.widgets.active.rounding = r;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.button_padding = vec2(12.0, 6.0);
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.animation_time = 0.12;
    });
}

fn new_cmd<S: AsRef<std::ffi::OsStr>>(p: S) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(p);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

#[cfg(not(windows))]
fn shell_quote(p: &std::path::Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\''"))
}

#[cfg(windows)]
fn shell_quote(p: &std::path::Path) -> String {
    format!("\"{}\"", p.display())
}

#[cfg(not(windows))]
fn shell_cmd(cmd: String) -> Command {
    let mut c = new_cmd("sh");
    c.arg("-c").arg(cmd);
    c
}

#[cfg(windows)]
fn shell_cmd(cmd: String) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = new_cmd("cmd");
    c.raw_arg("/C").raw_arg(cmd);
    c
}

#[cfg(not(windows))]
fn appimage_cmd(g: &Game) -> Command {
    if let Ok(m) = fs::metadata(&g.exe) {
        let mut perm = m.permissions();
        if perm.mode() & 0o111 == 0 {
            perm.set_mode(perm.mode() | 0o111);
            let _ = fs::set_permissions(&g.exe, perm);
        }
    }
    Command::new(&g.exe)
}

#[cfg(windows)]
fn appimage_cmd(g: &Game) -> Command {
    Command::new(&g.exe)
}

fn wd(args: &[&str]) -> Option<String> {
    new_cmd("waydroid")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn wd_packages() -> Vec<String> {
    wd(&["app", "list"])
        .map(|o| {
            o.lines()
                .filter_map(|l| l.trim().strip_prefix("packageName:").map(|v| v.trim().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn apk_package(apk: &std::path::Path) -> Option<String> {
    let o = new_cmd("aapt").args(["dump", "badging"]).arg(apk).output().ok()?;
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    let l = t.lines().find(|l| l.starts_with("package:"))?;
    let i = l.find("name='")? + 6;
    let rest = &l[i..];
    Some(rest[..rest.find('\'')?].to_string())
}

fn wsl_run(args: &[&str]) -> Option<String> {
    new_cmd("wsl.exe")
        .arg("-e")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn run_wsl_appimage(exe: PathBuf, name: String, tx: Sender<Note>, ctx: egui::Context) {
    let say = |m: &str| {
        let _ = tx.send(Note::Status(m.to_string()));
        ctx.request_repaint();
    };
    let win = exe.to_string_lossy().to_string();
    let Some(lin) = wsl_run(&["wslpath", "-u", &win])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        say("WSL not available, install WSL2 with a Linux distro");
        return;
    };
    let mut target = lin.clone();
    if wsl_run(&["test", "-x", &lin]).is_none() {
        say("Preparing AppImage in WSL...");
        let h = lin.bytes().fold(0u64, |a, b| a.wrapping_mul(131).wrapping_add(b as u64));
        let tmp = format!("/tmp/zl_{h}.AppImage");
        if wsl_run(&["cp", "-u", &lin, &tmp]).is_none() || wsl_run(&["chmod", "+x", &tmp]).is_none() {
            say("Could not copy AppImage into WSL");
            return;
        }
        target = tmp;
    }
    say(&format!("Starting {name}..."));
    match new_cmd("wsl.exe")
        .args(["-e", target.as_str(), "--appimage-extract-and-run"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(mut ch) => {
            std::thread::spawn(move || {
                let _ = ch.wait();
            });
            say("Started in WSL");
        }
        Err(e) => say(&format!("Error: {e}")),
    }
}

fn adb(bin: &str, args: &[&str]) -> Option<String> {
    new_cmd(bin)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn adb_ready(bin: &str) -> bool {
    adb(bin, &["devices"])
        .map(|o| o.lines().any(|l| l.trim_end().ends_with("\tdevice")))
        .unwrap_or(false)
}

fn adb_packages(bin: &str) -> Vec<String> {
    adb(bin, &["shell", "pm", "list", "packages", "-3"])
        .map(|o| {
            o.lines()
                .filter_map(|l| l.trim().strip_prefix("package:").map(|v| v.trim().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn run_adb(apk: PathBuf, pkg: Option<String>, s: Settings, tx: Sender<Note>, ctx: egui::Context) {
    let say = |m: &str| {
        let _ = tx.send(Note::Status(m.to_string()));
        ctx.request_repaint();
    };
    let bin = if s.adb_bin.trim().is_empty() { "adb" } else { s.adb_bin.trim() }.to_string();
    if new_cmd(&bin).arg("version").output().is_err() {
        say("adb not found, set its path in Settings");
        return;
    }
    if !s.adb_connect.trim().is_empty() {
        let _ = adb(&bin, &["connect", s.adb_connect.trim()]);
    }
    if !adb_ready(&bin) {
        if s.avd.trim().is_empty() {
            say("No Android device connected, start an emulator or set an AVD name");
            return;
        }
        say("Starting emulator...");
        let emu = if s.emulator_bin.trim().is_empty() { "emulator" } else { s.emulator_bin.trim() };
        match new_cmd(emu)
            .args(["-avd", s.avd.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut ch) => {
                std::thread::spawn(move || {
                    let _ = ch.wait();
                });
            }
            Err(e) => {
                say(&format!("Emulator error: {e}"));
                return;
            }
        }
        let mut up = false;
        for _ in 0..160 {
            std::thread::sleep(Duration::from_secs(1));
            if adb_ready(&bin)
                && adb(&bin, &["shell", "getprop", "sys.boot_completed"])
                    .map(|o| o.trim() == "1")
                    .unwrap_or(false)
            {
                up = true;
                break;
            }
        }
        if !up {
            say("Emulator did not start in time");
            return;
        }
    }
    let pkg = match pkg {
        Some(p) => p,
        None => {
            say("Installing APK...");
            let before = adb_packages(&bin);
            let found = apk_package(&apk);
            let ok = new_cmd(&bin)
                .args(["install", "-r"])
                .arg(&apk)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !ok {
                say("APK install failed");
                return;
            }
            let after = adb_packages(&bin);
            let p = found.or_else(|| after.into_iter().find(|p| !before.contains(p)));
            match p {
                Some(p) => {
                    let _ = tx.send(Note::Package(apk.clone(), p.clone()));
                    p
                }
                None => {
                    say("Installed, but package name could not be detected");
                    return;
                }
            }
        }
    };
    say(&format!("Launching {pkg}..."));
    let ok = adb(
        &bin,
        &["shell", "monkey", "-p", &pkg, "-c", "android.intent.category.LAUNCHER", "1"],
    )
    .is_some();
    say(if ok { "Android app started" } else { "Failed to launch Android app" });
}

fn run_waydroid(apk: PathBuf, pkg: Option<String>, tx: Sender<Note>, ctx: egui::Context) {
    let say = |m: &str| {
        let _ = tx.send(Note::Status(m.to_string()));
        ctx.request_repaint();
    };
    let session_up = || {
        wd(&["status"])
            .map(|o| o.lines().any(|l| l.starts_with("Session:") && l.contains("RUNNING")))
            .unwrap_or(false)
    };
    if new_cmd("waydroid").arg("status").output().is_err() {
        say("waydroid not found, install it first");
        return;
    }
    if !session_up() {
        say("Starting Waydroid session...");
        if let Ok(mut ch) = new_cmd("waydroid")
            .args(["session", "start"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            std::thread::spawn(move || {
                let _ = ch.wait();
            });
        }
        let mut up = false;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(750));
            if session_up() {
                up = true;
                break;
            }
        }
        if !up {
            say("Waydroid session did not start");
            return;
        }
    }
    let pkg = match pkg {
        Some(p) => p,
        None => {
            say("Installing APK...");
            let before = wd_packages();
            let found = apk_package(&apk);
            let ok = new_cmd("waydroid")
                .args(["app", "install"])
                .arg(&apk)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !ok {
                say("APK install failed");
                return;
            }
            let after = wd_packages();
            let p = found.or_else(|| after.into_iter().find(|p| !before.contains(p)));
            match p {
                Some(p) => {
                    let _ = tx.send(Note::Package(apk.clone(), p.clone()));
                    p
                }
                None => {
                    say("Installed, but package name could not be detected");
                    return;
                }
            }
        }
    };
    say(&format!("Launching {pkg}..."));
    let ok = new_cmd("waydroid")
        .args(["app", "launch", &pkg])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    say(if ok { "Android app started" } else { "Failed to launch Android app" });
}

#[cfg(windows)]
fn exe_cmd(_: &Settings, g: &Game) -> Command {
    Command::new(&g.exe)
}

#[cfg(not(windows))]
fn exe_cmd(s: &Settings, g: &Game) -> Command {
    let bin = if s.wine_bin.trim().is_empty() { "wine" } else { s.wine_bin.trim() };
    let mut c = new_cmd(bin);
    c.arg(&g.exe);
    if let Some(p) = g.prefix.as_ref().or(s.default_prefix.as_ref()) {
        c.env("WINEPREFIX", p);
    }
    if s.esync {
        c.env("WINEESYNC", "1");
    }
    if s.fsync {
        c.env("WINEFSYNC", "1");
    }
    if s.quiet {
        c.env("WINEDEBUG", "-all");
    }
    if s.dxvk_hud {
        c.env("DXVK_HUD", "fps");
    }
    c
}

impl App {
    fn load() -> Self {
        let old = dirs::config_dir().unwrap_or_default().join("winelauncher");
        let new = dir();
        let migrated = old.exists() && !new.exists() && fs::rename(&old, &new).is_ok();
        let mut games: Vec<Game> = fs::read_to_string(dir().join("games.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut settings: Settings = fs::read_to_string(dir().join("settings.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        if migrated {
            for g in games.iter_mut() {
                if let Some(Ok(rest)) = g.icon.as_ref().map(|p| p.strip_prefix(&old).map(|r| r.to_path_buf())) {
                    g.icon = Some(new.join(rest));
                }
            }
            if let Some(Ok(rest)) = settings
                .bg_image
                .as_ref()
                .map(|p| p.strip_prefix(&old).map(|r| r.to_path_buf()))
            {
                settings.bg_image = Some(new.join(rest));
            }
        }
        if settings.icon_size == 128.0 || settings.icon_size == 160.0 {
            settings.icon_size = 220.0;
        }
        let transparent_at_start = settings.transparent;
        let (tx, rx) = mpsc::channel();
        let (note_tx, note_rx) = mpsc::channel();
        Self {
            tx,
            rx,
            note_tx,
            note_rx,
            busy: false,
            transparent_at_start,
            games,
            settings,
            search: String::new(),
            show_settings: false,
            status: String::new(),
        }
    }

    fn accent(&self) -> Color32 {
        let [r, g, b] = self.settings.accent;
        Color32::from_rgb(r, g, b)
    }

    fn persist(&self) {
        let d = dir();
        let _ = fs::create_dir_all(&d);
        if let Ok(s) = serde_json::to_string_pretty(&self.games) {
            let _ = fs::write(d.join("games.json"), s);
        }
    }

    fn persist_settings(&self) {
        let d = dir();
        let _ = fs::create_dir_all(&d);
        if let Ok(s) = serde_json::to_string_pretty(&self.settings) {
            let _ = fs::write(d.join("settings.json"), s);
        }
    }

    fn add_path(&mut self, p: PathBuf) {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let kind = Kind::from_path(&p);
        self.games.push(Game { name, exe: p, prefix: None, icon: None, kind, package: None });
        self.persist();
    }

    fn request(&mut self, ctx: &egui::Context, t: Target) {
        if self.busy {
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = match t {
                Target::Add => rfd::FileDialog::new()
                    .add_filter(
                        "Games",
                        &["exe", "EXE", "AppImage", "appimage", "jar", "JAR", "apk", "APK"],
                    )
                    .pick_file(),
                Target::Icon(_) | Target::BgImage => rfd::FileDialog::new()
                    .add_filter("image", &["png", "PNG", "jpg", "jpeg", "JPG", "JPEG"])
                    .pick_file(),
                Target::Prefix(_) | Target::DefPrefix => rfd::FileDialog::new().pick_folder(),
            };
            let _ = tx.send((t, r));
            ctx.request_repaint();
        });
    }

    fn launch(&mut self, ctx: &egui::Context, g: &Game) -> bool {
        let s = &self.settings;
        let mut c = match g.kind {
            Kind::Exe => exe_cmd(s, g),
            Kind::AppImage => {
                if cfg!(windows) {
                    let exe = g.exe.clone();
                    let name = g.name.clone();
                    let tx = self.note_tx.clone();
                    let ctx = ctx.clone();
                    self.status = format!("Starting {}...", g.name);
                    std::thread::spawn(move || run_wsl_appimage(exe, name, tx, ctx));
                    return false;
                }
                appimage_cmd(g)
            }
            Kind::Apk => match s.apk_mode {
                ApkMode::Waydroid => {
                    let apk = g.exe.clone();
                    let pkg = g.package.clone();
                    let tx = self.note_tx.clone();
                    let ctx = ctx.clone();
                    self.status = format!("Starting {}...", g.name);
                    std::thread::spawn(move || run_waydroid(apk, pkg, tx, ctx));
                    return false;
                }
                ApkMode::Adb => {
                    let apk = g.exe.clone();
                    let pkg = g.package.clone();
                    let st = s.clone();
                    let tx = self.note_tx.clone();
                    let ctx = ctx.clone();
                    self.status = format!("Starting {}...", g.name);
                    std::thread::spawn(move || run_adb(apk, pkg, st, tx, ctx));
                    return false;
                }
                ApkMode::Custom => {
                    let cmd = s
                        .apk_cmd
                        .replace("{apk}", &shell_quote(&g.exe))
                        .replace("{pkg}", g.package.as_deref().unwrap_or(""));
                    shell_cmd(cmd)
                }
            },
            Kind::Jar => {
                let bin = if s.java_bin.trim().is_empty() { "java" } else { s.java_bin.trim() };
                let mut c = new_cmd(bin);
                c.arg("-jar").arg(&g.exe);
                c
            }
        };
        c.stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(d) = g.exe.parent() {
            c.current_dir(d);
        }
        let r = c.spawn();
        let ok = r.is_ok();
        self.status = match r {
            Ok(mut ch) => {
                std::thread::spawn(move || {
                    let _ = ch.wait();
                });
                format!("Started {}", g.name)
            }
            Err(e) => format!("Error: {e}"),
        };
        ok
    }
}

impl eframe::App for App {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        if self.transparent_at_start {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            let [r, g, b] = self.settings.bg_color;
            Color32::from_rgb(r, g, b).to_normalized_gamma_f32()
        }
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        while let Ok(n) = self.note_rx.try_recv() {
            match n {
                Note::Status(m) => self.status = m,
                Note::Package(apk, pkg) => {
                    if let Some(g) = self.games.iter_mut().find(|g| g.exe == apk) {
                        g.package = Some(pkg);
                    }
                    self.persist();
                }
            }
        }
        while let Ok((t, r)) = self.rx.try_recv() {
            self.busy = false;
            if let Some(p) = r {
                match t {
                    Target::Add => self.add_path(p),
                    Target::Icon(i) => {
                        if let Some(g) = self.games.get_mut(i) {
                            g.icon = Some(import_icon(p));
                            self.persist();
                        }
                    }
                    Target::Prefix(i) => {
                        if let Some(g) = self.games.get_mut(i) {
                            g.prefix = Some(p);
                            self.persist();
                        }
                    }
                    Target::BgImage => {
                        self.settings.bg_image = Some(import_icon(p));
                        self.persist_settings();
                    }
                    Target::DefPrefix => {
                        self.settings.default_prefix = Some(p);
                        self.persist_settings();
                    }
                }
            }
        }

        {
            let screen = ctx.screen_rect();
            let st = &self.settings;
            let k = if self.transparent_at_start && st.transparent { st.opacity } else { 1.0 };
            let [r, g, b] = st.bg_color;
            let bp = ctx.layer_painter(egui::LayerId::background());
            bp.rect_filled(screen, 0.0, Color32::from_rgb(r, g, b).gamma_multiply(k));
            if let Some(path) = &st.bg_image {
                let uri = format!("file://{}", path.display());
                if let Ok(egui::load::TexturePoll::Ready { texture }) = ctx.try_load_texture(
                    &uri,
                    egui::TextureOptions::LINEAR,
                    egui::load::SizeHint::default(),
                ) {
                    let ts = texture.size;
                    let rs = screen.size();
                    let sc = (rs.x / ts.x).max(rs.y / ts.y);
                    let uv = Rect::from_center_size(
                        egui::pos2(0.5, 0.5),
                        vec2(rs.x / sc / ts.x, rs.y / sc / ts.y),
                    );
                    bp.image(texture.id, screen, uv, Color32::WHITE.gamma_multiply(k));
                    bp.rect_filled(screen, 0.0, Color32::from_black_alpha((st.bg_dim * 255.0 * k) as u8));
                }
            }
        }
        let accent = self.accent();
        let target_size = self.settings.icon_size;
        let sw = ctx.screen_rect().width();
        let narrow = sw < 640.0;
        let m = if sw < 520.0 { 12.0 } else { 24.0 };
        let mut launch = None;
        let mut remove = None;
        let mut dirty = false;
        let mut add = false;
        let mut req: Option<Target> = None;

        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::symmetric(m, 14.0)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        let (title, ts) = if sw < 480.0 { ("Zimn", 20.0) } else { ("ZimnLauncher", 26.0) };
                        ui.label(RichText::new(title).size(ts).strong());
                        ui.label(RichText::new(format!("{} games", self.games.len())).weak());
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("+ Add game").strong().color(Color32::WHITE))
                                    .fill(accent),
                            )
                            .clicked()
                        {
                            add = true;
                        }
                        if ui.button("Settings").clicked() {
                            self.show_settings = !self.show_settings;
                        }
                        if !narrow {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search)
                                    .hint_text("Search...")
                                    .desired_width(200.0)
                                    .margin(vec2(10.0, 7.0)),
                            );
                        }
                    });
                });
                if narrow {
                    ui.add_space(4.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search...")
                            .desired_width(ui.available_width())
                            .margin(vec2(10.0, 7.0)),
                    );
                }
            });

        egui::TopBottomPanel::bottom("bottom")
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::symmetric(m, 8.0)))
            .show(ctx, |ui| {
                ui.label(RichText::new(&self.status).weak());
            });

        let q = self.search.to_lowercase();
        let mut order: Vec<usize> = (0..self.games.len())
            .filter(|&i| q.is_empty() || self.games[i].name.to_lowercase().contains(&q))
            .collect();
        if self.settings.sort {
            order.sort_by_key(|&i| self.games[i].name.to_lowercase());
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::symmetric(m, 8.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.games.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("No games yet").size(22.0).weak());
                                ui.label(RichText::new("Add an .exe, .AppImage or .jar to get started").weak());
                            });
                            ui.add_space(20.0);
                        }

                        let gap = 28.0;
                        let avail = ui.available_width();
                        let size = target_size.min(avail).max(96.0);
                        let outer = size;
                        let cols = (((avail + gap) / (outer + gap)).floor() as usize).max(1);
                        let used = cols as f32 * outer + (cols as f32 - 1.0) * gap;
                        let pad = ((avail - used) / 2.0).max(0.0);

                        let mut items: Vec<Option<usize>> = order.iter().map(|&i| Some(i)).collect();
                        if q.is_empty() {
                            items.push(None);
                        }

                        ui.spacing_mut().item_spacing = vec2(gap, gap);
                        for row in items.chunks(cols) {
                            ui.horizontal(|ui| {
                                ui.add_space(pad);
                                for it in row {
                                    let i = match *it {
                                        Some(i) => i,
                                        None => {
                                            if ui
                                                .add_sized(
                                                    [outer, outer * 1.5],
                                                    egui::Button::new(
                                                        RichText::new("+").size(56.0).color(Color32::GRAY),
                                                    )
                                                    .fill(CARD)
                                                    .rounding(18.0),
                                                )
                                                .clicked()
                                            {
                                                add = true;
                                            }
                                            continue;
                                        }
                                    };
                                    let g = &mut self.games[i];
                                    let hid = egui::Id::new(("card", &g.exe));
                                    let prev = ctx.data(|d| d.get_temp::<bool>(hid)).unwrap_or(false);
                                    let t = ctx.animate_bool_with_time(hid.with("anim"), prev, 0.16);
                                    let t = t * t * (3.0 - 2.0 * t);

                                    let (slot, resp) = ui.allocate_exact_size(vec2(size, size * 1.5), Sense::click());
                                    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                                    let r = Rect::from_center_size(
                                        slot.center() - vec2(0.0, 4.0 * t),
                                        slot.size() * (1.0 + 0.04 * t),
                                    );
                                    let p = ui.painter().clone();

                                    p.add(
                                        egui::epaint::Shadow {
                                            offset: vec2(0.0, 4.0 + 6.0 * t),
                                            blur: 14.0 + 14.0 * t,
                                            spread: 0.0,
                                            color: Color32::from_black_alpha((90.0 + 70.0 * t) as u8),
                                        }
                                        .as_shape(r, 16.0),
                                    );
                                    p.rect_filled(r, 16.0, CARD);
                                    match &g.icon {
                                        Some(path) => {
                                            let uri = format!("file://{}", path.display());
                                            match ctx.try_load_texture(
                                                &uri,
                                                egui::TextureOptions::LINEAR,
                                                egui::load::SizeHint::default(),
                                            ) {
                                                Ok(egui::load::TexturePoll::Ready { texture }) => {
                                                    let ts = texture.size;
                                                    let rs = r.size();
                                                    let sc = (rs.x / ts.x).max(rs.y / ts.y);
                                                    let uv = Rect::from_center_size(
                                                        egui::pos2(0.5, 0.5),
                                                        vec2(rs.x / sc / ts.x, rs.y / sc / ts.y),
                                                    );
                                                    egui::Image::from_texture(texture)
                                                        .uv(uv)
                                                        .rounding(16.0)
                                                        .paint_at(ui, r);
                                                }
                                                Err(_) => placeholder(ui, &g.name, r),
                                                _ => {}
                                            }
                                        }
                                        None => placeholder(ui, &g.name, r),
                                    }

                                    if t > 0.01 {
                                        p.rect_filled(r, 16.0, Color32::from_black_alpha((110.0 * t) as u8));
                                        let c = r.center() - vec2(0.0, r.height() * 0.05);
                                        let s = r.width() * 0.14;
                                        p.add(egui::Shape::convex_polygon(
                                            vec![
                                                egui::pos2(c.x - s * 0.7, c.y - s),
                                                egui::pos2(c.x - s * 0.7, c.y + s),
                                                egui::pos2(c.x + s, c.y),
                                            ],
                                            Color32::from_white_alpha((235.0 * t) as u8),
                                            Stroke::NONE,
                                        ));
                                    }

                                    let bh = (r.height() * 0.15).clamp(36.0, 56.0);
                                    let band = Rect::from_min_max(egui::pos2(r.min.x, r.max.y - bh), r.max);
                                    p.rect_filled(
                                        band,
                                        egui::Rounding { nw: 0.0, ne: 0.0, sw: 16.0, se: 16.0 },
                                        Color32::from_black_alpha(150 + (40.0 * t) as u8),
                                    );
                                    let mut job = egui::text::LayoutJob::simple(
                                        g.name.clone(),
                                        FontId::proportional(16.0),
                                        Color32::WHITE,
                                        r.width() - 20.0,
                                    );
                                    job.wrap.max_rows = 1;
                                    job.wrap.break_anywhere = true;
                                    let galley = ui.fonts(|f| f.layout_job(job));
                                    let pos = egui::pos2(
                                        r.center().x - galley.size().x / 2.0,
                                        band.center().y - galley.size().y / 2.0,
                                    );
                                    p.galley(pos, galley, Color32::WHITE);

                                    let badge = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            g.kind.label().to_string(),
                                            FontId::proportional(10.0),
                                            Color32::from_white_alpha(230),
                                        )
                                    });
                                    let pill = Rect::from_min_size(r.min + vec2(8.0, 8.0), badge.size() + vec2(12.0, 6.0));
                                    p.rect_filled(pill, 8.0, Color32::from_black_alpha(150));
                                    p.galley(pill.min + vec2(6.0, 3.0), badge, Color32::WHITE);

                                    p.rect_stroke(
                                        r,
                                        16.0,
                                        Stroke::new(1.0 + 1.2 * t, accent.gamma_multiply(0.12 + 0.8 * t)),
                                    );

                                    if resp.clicked() {
                                        launch = Some(i);
                                    }
                                    let is_exe = g.kind == Kind::Exe;
                                    resp.context_menu(|ui| {
                                        ui.set_min_width(170.0);
                                        if ui.button("Play").clicked() {
                                            launch = Some(i);
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        ui.label(RichText::new("Name").small().weak());
                                        if ui
                                            .add(egui::TextEdit::singleline(&mut g.name).desired_width(150.0))
                                            .changed()
                                        {
                                            dirty = true;
                                        }
                                        ui.separator();
                                        if ui.button("Change icon...").clicked() {
                                            req = Some(Target::Icon(i));
                                            ui.close_menu();
                                        }
                                        if g.icon.is_some() && ui.button("Remove icon").clicked() {
                                            g.icon = None;
                                            dirty = true;
                                            ui.close_menu();
                                        }
                                        if ui
                                            .add_enabled(is_exe, egui::Button::new("Wine prefix..."))
                                            .on_disabled_hover_text("Wine only")
                                            .on_hover_text(
                                                g.prefix
                                                    .as_ref()
                                                    .map(|p| p.display().to_string())
                                                    .unwrap_or_else(|| "default prefix".into()),
                                            )
                                            .clicked()
                                        {
                                            req = Some(Target::Prefix(i));
                                            ui.close_menu();
                                        }
                                        if is_exe && g.prefix.is_some() && ui.button("Reset prefix").clicked() {
                                            g.prefix = None;
                                            dirty = true;
                                            ui.close_menu();
                                        }
                                        if g.kind == Kind::Apk
                                            && g.package.is_some()
                                            && ui.button("Reinstall on next launch").clicked()
                                        {
                                            g.package = None;
                                            dirty = true;
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui
                                            .button(RichText::new("Remove").color(Color32::from_rgb(230, 100, 100)))
                                            .clicked()
                                        {
                                            remove = Some(i);
                                            ui.close_menu();
                                        }
                                    });
                                    ctx.data_mut(|d| d.insert_temp(hid, resp.hovered()));
                                }
                            });
                        }
                    });
            });

        let mut changed = false;
        let mut restyle = false;
        let fw = (sw - 200.0).clamp(120.0, 240.0);
        egui::Window::new("Settings")
            .open(&mut self.show_settings)
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .max_size(ctx.screen_rect().size() - vec2(24.0, 24.0))
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                egui::Grid::new("settings_grid")
                    .num_columns(2)
                    .spacing([18.0, 12.0])
                    .show(ui, |ui| {
                        ui.label("Wine binary");
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut self.settings.wine_bin).desired_width(fw))
                            .changed();
                        ui.end_row();

                        ui.label("Java binary");
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut self.settings.java_bin).desired_width(fw))
                            .changed();
                        ui.end_row();

                        ui.label("APK runner");
                        ui.horizontal(|ui| {
                            if !cfg!(windows) {
                                changed |= ui
                                    .selectable_value(&mut self.settings.apk_mode, ApkMode::Waydroid, "Waydroid")
                                    .changed();
                            }
                            changed |= ui
                                .selectable_value(&mut self.settings.apk_mode, ApkMode::Adb, "ADB")
                                .changed();
                            changed |= ui
                                .selectable_value(&mut self.settings.apk_mode, ApkMode::Custom, "Custom")
                                .changed();
                        });
                        ui.end_row();

                        if self.settings.apk_mode == ApkMode::Adb {
                            ui.label("adb binary");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.settings.adb_bin)
                                        .hint_text("adb")
                                        .desired_width(fw),
                                )
                                .changed();
                            ui.end_row();

                            ui.label("adb connect");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.settings.adb_connect)
                                        .hint_text("127.0.0.1:5555 (optional)")
                                        .desired_width(fw),
                                )
                                .changed();
                            ui.end_row();

                            ui.label("Emulator binary");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.settings.emulator_bin)
                                        .hint_text("emulator")
                                        .desired_width(fw),
                                )
                                .changed();
                            ui.end_row();

                            ui.label("AVD name");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.settings.avd)
                                        .hint_text("Pixel_6 (optional)")
                                        .desired_width(fw),
                                )
                                .changed();
                            ui.end_row();
                        }

                        if self.settings.apk_mode == ApkMode::Custom {
                            ui.label("APK command");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut self.settings.apk_cmd)
                                        .hint_text("my-emulator {apk}")
                                        .desired_width(fw),
                                )
                                .changed();
                            ui.end_row();
                        }

                        ui.label("Default prefix");
                        ui.horizontal(|ui| {
                            let t = self
                                .settings
                                .default_prefix
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "~/.wine".into());
                            ui.label(RichText::new(t).weak());
                            if ui.button("Browse").clicked() {
                                req = Some(Target::DefPrefix);
                            }
                            if ui.button("Reset").clicked() {
                                self.settings.default_prefix = None;
                                changed = true;
                            }
                        });
                        ui.end_row();

                        ui.label("Icon size");
                        changed |= ui
                            .add(egui::Slider::new(&mut self.settings.icon_size, 120.0..=400.0).suffix(" px"))
                            .changed();
                        ui.end_row();

                        ui.label("Accent color");
                        if ui.color_edit_button_srgb(&mut self.settings.accent).changed() {
                            changed = true;
                            restyle = true;
                        }
                        ui.end_row();

                        ui.label("Background color");
                        changed |= ui.color_edit_button_srgb(&mut self.settings.bg_color).changed();
                        ui.end_row();

                        ui.label("Background image");
                        ui.horizontal(|ui| {
                            let t = self
                                .settings
                                .bg_image
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "none".into());
                            ui.label(RichText::new(t).weak());
                            if ui.button("Browse").clicked() {
                                req = Some(Target::BgImage);
                            }
                            if ui.button("Clear").clicked() {
                                self.settings.bg_image = None;
                                changed = true;
                            }
                        });
                        ui.end_row();

                        ui.label("Image dimming");
                        let has_img = self.settings.bg_image.is_some();
                        changed |= ui
                            .add_enabled(has_img, egui::Slider::new(&mut self.settings.bg_dim, 0.0..=0.9))
                            .changed();
                        ui.end_row();

                        ui.label("Transparent window");
                        changed |= ui
                            .checkbox(&mut self.settings.transparent, "needs restart")
                            .changed();
                        ui.end_row();

                        ui.label("Window opacity");
                        let tr = self.settings.transparent;
                        changed |= ui
                            .add_enabled(tr, egui::Slider::new(&mut self.settings.opacity, 0.2..=1.0))
                            .changed();
                        ui.end_row();
                    });
                ui.separator();
                changed |= ui.checkbox(&mut self.settings.esync, "WINEESYNC").changed();
                changed |= ui.checkbox(&mut self.settings.fsync, "WINEFSYNC").changed();
                changed |= ui.checkbox(&mut self.settings.quiet, "Silence wine debug output").changed();
                changed |= ui.checkbox(&mut self.settings.dxvk_hud, "DXVK FPS overlay").changed();
                changed |= ui.checkbox(&mut self.settings.close_on_launch, "Close launcher after start").changed();
                changed |= ui.checkbox(&mut self.settings.sort, "Sort A-Z").changed();
            });
        if restyle {
            style(ctx, self.accent());
        }
        if changed {
            self.persist_settings();
        }

        if add {
            req = Some(Target::Add);
        }
        if let Some(t) = req {
            self.request(ctx, t);
        }
        if let Some(i) = launch {
            let g = self.games[i].clone();
            if self.launch(ctx, &g) && self.settings.close_on_launch {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if let Some(i) = remove {
            self.games.remove(i);
            dirty = true;
        }
        if dirty {
            self.persist();
        }
    }
}

fn main() -> eframe::Result {
    let app = App::load();
    let transparent = app.transparent_at_start;
    eframe::run_native(
        "ZimnLauncher",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1000.0, 700.0])
                .with_app_id("zimnlauncher")
                .with_min_inner_size([340.0, 360.0])
                .with_transparent(transparent),
            ..Default::default()
        },
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            style(&cc.egui_ctx, app.accent());
            Ok(Box::new(app))
        }),
    )
}
