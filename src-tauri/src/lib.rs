//! Replica Works デスクトップ版（Mac / Windows）。
//! 本番 URL をネイティブ窓で表示する薄いシェル。入口はお客様ページのログイン画面で、
//! 管理者はログイン後にメニュー「管理画面に切り替え」で管理画面へ入る（Web 側の /portal/switch）。
//! Web 側には Tauri API を渡さない。ローカル HTML（ui/）は接続できなかったときの案内だけ。

use std::{
  collections::HashMap,
  path::PathBuf,
  sync::Mutex,
};

use tauri::{
  menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
  webview::{DownloadEvent, NewWindowResponse},
  AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// 本番 URL。ローカル確認は `REPLICA_WORKS_BASE_URL=http://localhost:3000 npm run dev` で差し替える
const BASE_URL: &str = match option_env!("REPLICA_WORKS_BASE_URL") {
  Some(v) => v,
  None => "https://replica-works--replica-d2b34.asia-east1.hosted.app",
};

/// アプリ内（この窓）で開いてよいホスト。ログイン途中の Google / Firebase もここに含める。
/// それ以外の http(s) と mailto: などは OS の既定ブラウザ・アプリに渡す
fn stays_inside(url: &Url) -> bool {
  match url.scheme() {
    "tauri" | "about" | "blob" | "data" => return true,
    "http" | "https" => {}
    _ => return false,
  }
  let Some(host) = url.host_str() else { return false };
  if let Ok(base) = Url::parse(BASE_URL) {
    if base.host_str() == Some(host) {
      return true;
    }
  }
  matches!(host, "localhost" | "127.0.0.1" | "tauri.localhost")
    || host == "accounts.google.com"
    || host.ends_with(".google.com")
    || host.ends_with(".googleapis.com")
    || host.ends_with(".gstatic.com")
    || host.ends_with(".firebaseapp.com")
    || host.ends_with(".web.app")
}

fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) {
  if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
    log::warn!("外部で開けませんでした {url}: {e}");
  }
}

/// ダウンロード先。macOS は完了イベントにパスが乗らないので、開始時に覚えておく
struct Downloads(Mutex<HashMap<String, PathBuf>>);
struct Zoom(Mutex<f64>);

/// 起動時に開くページ。お客様ページのログインが入口で、管理者はログイン後にメニューの「管理画面に切り替え」で管理画面へ
fn start_url() -> Url {
  Url::parse(&format!("{}/portal", BASE_URL.trim_end_matches('/'))).expect("BASE_URL")
}

fn navigate(win: &WebviewWindow, url: Url) {
  if let Err(e) = win.navigate(url) {
    log::warn!("移動できませんでした: {e}");
  }
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
  let info = app.package_info();
  let about = AboutMetadata {
    name: Some(info.name.clone()),
    version: Some(info.version.to_string()),
    copyright: app.config().bundle.copyright.clone(),
    authors: app.config().bundle.publisher.clone().map(|p| vec![p]),
    ..Default::default()
  };

  let view = Submenu::with_items(
    app,
    "表示",
    true,
    &[
      &MenuItem::with_id(app, "home", "ホーム", true, Some("CmdOrCtrl+Shift+H"))?,
      &PredefinedMenuItem::separator(app)?,
      &MenuItem::with_id(app, "back", "戻る", true, Some("CmdOrCtrl+["))?,
      &MenuItem::with_id(app, "forward", "進む", true, Some("CmdOrCtrl+]"))?,
      &MenuItem::with_id(app, "reload", "再読み込み", true, Some("CmdOrCtrl+R"))?,
      &PredefinedMenuItem::separator(app)?,
      &MenuItem::with_id(app, "zoom-in", "拡大", true, Some("CmdOrCtrl+="))?,
      &MenuItem::with_id(app, "zoom-out", "縮小", true, Some("CmdOrCtrl+-"))?,
      &MenuItem::with_id(app, "zoom-reset", "等倍", true, Some("CmdOrCtrl+Shift+0"))?,
      &PredefinedMenuItem::separator(app)?,
      &PredefinedMenuItem::fullscreen(app, Some("フルスクリーン"))?,
    ],
  )?;

  let edit = Submenu::with_items(
    app,
    "編集",
    true,
    &[
      &PredefinedMenuItem::undo(app, Some("取り消す"))?,
      &PredefinedMenuItem::redo(app, Some("やり直す"))?,
      &PredefinedMenuItem::separator(app)?,
      &PredefinedMenuItem::cut(app, Some("カット"))?,
      &PredefinedMenuItem::copy(app, Some("コピー"))?,
      &PredefinedMenuItem::paste(app, Some("ペースト"))?,
      &PredefinedMenuItem::select_all(app, Some("すべてを選択"))?,
    ],
  )?;

  let window = Submenu::with_items(
    app,
    "ウインドウ",
    true,
    &[
      &PredefinedMenuItem::minimize(app, Some("しまう"))?,
      &PredefinedMenuItem::maximize(app, Some("拡大/縮小"))?,
      &PredefinedMenuItem::separator(app)?,
      &PredefinedMenuItem::close_window(app, Some("閉じる"))?,
    ],
  )?;

  let help = Submenu::with_items(
    app,
    "ヘルプ",
    true,
    &[
      &MenuItem::with_id(app, "open-web", "ブラウザで開く", true, None::<&str>)?,
      #[cfg(not(target_os = "macos"))]
      &PredefinedMenuItem::about(app, Some("Replica Works について"), Some(about.clone()))?,
    ],
  )?;

  #[cfg(target_os = "macos")]
  let menu = Menu::with_items(
    app,
    &[
      &Submenu::with_items(
        app,
        info.name.clone(),
        true,
        &[
          &PredefinedMenuItem::about(app, Some("Replica Works について"), Some(about))?,
          &PredefinedMenuItem::separator(app)?,
          &PredefinedMenuItem::services(app, Some("サービス"))?,
          &PredefinedMenuItem::separator(app)?,
          &PredefinedMenuItem::hide(app, Some("Replica Works を隠す"))?,
          &PredefinedMenuItem::hide_others(app, Some("ほかを隠す"))?,
          &PredefinedMenuItem::separator(app)?,
          &PredefinedMenuItem::quit(app, Some("Replica Works を終了"))?,
        ],
      )?,
      &edit,
      &view,
      &window,
      &help,
    ],
  )?;

  #[cfg(not(target_os = "macos"))]
  let menu = Menu::with_items(
    app,
    &[
      &Submenu::with_items(
        app,
        "ファイル",
        true,
        &[&PredefinedMenuItem::quit(app, Some("終了"))?],
      )?,
      &edit,
      &view,
      &window,
      &help,
    ],
  )?;

  Ok(menu)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
  let Some(win) = app.get_webview_window("main") else { return };
  match event.id().as_ref() {
    "home" => navigate(&win, start_url()),
    "back" => {
      let _ = win.eval("history.back()");
    }
    "forward" => {
      let _ = win.eval("history.forward()");
    }
    "reload" => {
      let _ = win.reload();
    }
    "zoom-in" | "zoom-out" | "zoom-reset" => {
      let state = app.state::<Zoom>();
      let mut z = state.0.lock().unwrap_or_else(|e| e.into_inner());
      *z = match event.id().as_ref() {
        "zoom-in" => (*z + 0.1).min(2.0),
        "zoom-out" => (*z - 0.1).max(0.5),
        _ => 1.0,
      };
      let _ = win.set_zoom(*z);
    }
    "open-web" => {
      if let Ok(u) = win.url() {
        // アプリ内のローカル画面（接続失敗など）なら入口を開く
        let target = if stays_inside(&u) && u.scheme() != "tauri" && u.host_str() != Some("tauri.localhost") {
          u
        } else {
          start_url()
        };
        open_external(app, &target);
      }
    }
    _ => {}
  }
}

fn user_agent(version: &str) -> String {
  // 埋め込み WebView と見なされると Google ログインが拒否されるので、OS 標準ブラウザ相当の UA に自前の印を足す。
  // Web 側は "ReplicaWorksDesktop" を見てデスクトップ向けの挙動（リダイレクト式ログインなど）に切り替える
  #[cfg(target_os = "macos")]
  let base = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.6 Safari/605.1.15";
  #[cfg(target_os = "windows")]
  let base = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0";
  #[cfg(not(any(target_os = "macos", target_os = "windows")))]
  let base = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
  format!("{base} ReplicaWorksDesktop/{version}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_window_state::Builder::default().build())
    .manage(Downloads(Mutex::new(HashMap::new())))
    .manage(Zoom(Mutex::new(1.0)))
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      let version = app.package_info().version.to_string();
      let platform = if cfg!(target_os = "macos") { "macos" } else if cfg!(target_os = "windows") { "windows" } else { "linux" };
      let init_script = format!(
        r#"window.__REPLICA_DESKTOP__ = {{ platform: "{platform}", version: "{version}", startUrl: "{base}/portal" }};
(function () {{ var f = function () {{ document.documentElement && document.documentElement.classList.add("replica-desktop"); }}; if (document.documentElement) f(); else document.addEventListener("DOMContentLoaded", f); }})();"#,
        base = BASE_URL.trim_end_matches('/'),
      );

      let nav_handle = app.handle().clone();
      let new_window_handle = app.handle().clone();
      let dl_handle = app.handle().clone();

      // 入口はお客様ページ（ログイン画面）。ui/index.html は接続できなかったときの案内にだけ使う
      let _win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(start_url()))
        .title("Replica Works")
        .inner_size(1280.0, 860.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .user_agent(&user_agent(&version))
        .initialization_script(init_script)
        .on_navigation(move |url| {
          if stays_inside(url) {
            true
          } else {
            open_external(&nav_handle, url);
            false
          }
        })
        .on_new_window(move |url, _features| {
          // target=_blank / window.open は常に既定ブラウザへ（PDF や外部サイト）
          open_external(&new_window_handle, &url);
          NewWindowResponse::Deny
        })
        .on_download(move |_webview, event| {
          match event {
            DownloadEvent::Requested { url, destination } => {
              // 保存先は OS の「ダウンロード」フォルダ。ファイル名は WebView が応答から決めたものを使う
              if destination.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
                if let Ok(dir) = dl_handle.path().download_dir() {
                  let name = destination.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "download".into());
                  *destination = dir.join(name);
                }
              }
              if let Ok(mut m) = dl_handle.state::<Downloads>().0.lock() {
                m.insert(url.to_string(), destination.clone());
              }
            }
            DownloadEvent::Finished { url, path, success } => {
              let remembered = dl_handle.state::<Downloads>().0.lock().ok().and_then(|mut m| m.remove(&url.to_string()));
              if success {
                if let Some(p) = path.or(remembered) {
                  // 保存できたらフォルダで見せる
                  let _ = dl_handle.opener().reveal_item_in_dir(&p);
                }
              }
            }
            _ => {}
          }
          true
        })
        .build()?;

      let menu = build_menu(app.handle())?;
      app.set_menu(menu)?;
      app.on_menu_event(on_menu);
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
