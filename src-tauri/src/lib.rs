//! Replica Works デスクトップ版（Mac / Windows）。
//! 本番 URL をネイティブ窓で表示する薄いシェル。入口はお客様ページのログイン画面で、
//! 管理者はログイン後にメニュー「管理画面に切り替え」で管理画面へ入る（Web 側の /portal/switch）。
//! Web 側には Tauri API を渡さない（Mac の窓を帯でドラッグする 2 つの命令だけ。overlay_titlebar）。起動時はローカル HTML（ui/）が接続を確かめてから入口へ移る。

use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{LazyLock, Mutex},
};

use tauri::{
  menu::{AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
  webview::{DownloadEvent, NewWindowResponse, PageLoadEvent},
  AppHandle, Manager, RunEvent, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// 本番 URL。ローカル確認は `REPLICA_WORKS_BASE_URL=http://localhost:3000 npm run dev` で差し替える。
/// 本番ビルドで差し替えるときは tauri.conf.json の csp（connect-src）も合わせる
const BASE_URL: &str = match option_env!("REPLICA_WORKS_BASE_URL") {
  Some(v) => v,
  None => "https://replica-works--replica-d2b34.asia-east1.hosted.app",
};
static BASE: LazyLock<Url> = LazyLock::new(|| Url::parse(BASE_URL).expect("BASE_URL"));

/// 同じオリジンのファイル（PDF など）を開く別窓のラベル。capabilities には載せない
const VIEWER: &str = "viewer";

/// Mac の main の窓は、タイトルバーを消して Web の上部の帯と一体にする（信号ボタンは帯の左に重なる）。
/// 帯は Web 側で data-tauri-drag-region を付け、そこをドラッグすると窓が動く・ダブルクリックで拡大する
const OVERLAY_TITLEBAR: bool = cfg!(target_os = "macos");
/// 窓を動かす命令だけを、main の窓の本番サイト（とローカル画面）に許す
const DRAG_PERMISSIONS: [&str; 2] = ["core:window:allow-start-dragging", "core:window:allow-internal-toggle-maximize"];

fn is_loopback(host: &str) -> bool {
  matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

/// 本番サイトと同じオリジンか（blob: は中のオリジンで比べる）
fn is_base(url: &Url) -> bool {
  url.origin() == BASE.origin()
}

/// 開発時だけ http の localhost を通す（デバッグビルド、または BASE_URL が http://localhost のとき）
fn dev_mode() -> bool {
  cfg!(debug_assertions) || (BASE.scheme() == "http" && BASE.host_str().is_some_and(is_loopback))
}

/// Google ログイン（リダイレクト式）の途中で通るホスト。authDomain は自ホストで /__/auth/* も自ホストが中継するので
/// firebaseapp.com / web.app は通らない。Google 側は accounts.google.com のほか SetSID や iframe（macOS は iframe の
/// 移動もここを通る）で accounts.youtube.com などを使うため、google.com / youtube.com のサブドメインは残す
fn is_google_signin_host(host: &str) -> bool {
  host == "accounts.google.com"
    || host == "accounts.google.co.jp"
    || host.ends_with(".google.com")
    || host.ends_with(".youtube.com")
}

/// アプリ内（この窓）で開いてよい URL。本番サイトと、ログイン途中の Google だけ（https のみ）。
/// それ以外の http(s) と mailto: / tel: は OS の既定ブラウザ・アプリに渡す
fn stays_inside(url: &Url) -> bool {
  if is_base(url) {
    return true;
  }
  let host = url.host_str().unwrap_or_default();
  match url.scheme() {
    // ローカル画面（macOS は tauri://localhost）と、iframe の about:blank / about:srcdoc
    "tauri" | "about" => true,
    // ローカル画面（Windows は http://tauri.localhost。WebView2 の中で完結し、通信はしない）
    "http" | "https" if host == "tauri.localhost" => true,
    "https" => is_google_signin_host(host),
    "http" => dev_mode() && is_loopback(host),
    _ => false,
  }
}

fn is_external_scheme(scheme: &str) -> bool {
  matches!(scheme, "http" | "https" | "mailto" | "tel")
}

/// OS の既定ブラウザ・アプリで開く。file: / smb: / shell: / ms-*: などは OS に渡さない
fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) {
  if !is_external_scheme(url.scheme()) {
    log::warn!("外部で開かない種類の URL を無視しました（{}:）", url.scheme());
    return;
  }
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

/// 次の起動で開き直してよいページ（ふつうの画面だけ。確認・同意・招待のトークン入りの URL、ファイル、API、ログインは覚えない）
const RESUME_EXACT: &[&str] = &["/", "/portal"];
const RESUME_PREFIX: &[&str] = &[
  "/projects", "/tasks", "/schedule", "/chat", "/contracts", "/customers", "/accounting", "/admins",
  "/portal/chat", "/portal/documents", "/portal/drawings", "/portal/media", "/portal/schedule", "/portal/account", "/portal/guide",
];
/// 覚えるクエリ（案件ページのタブ・顧客の検索・会計の年月）
const RESUME_QUERY: &[&str] = &["tab", "q", "y", "m"];
const LAST_PAGE_FILE: &str = "last-page.txt";

/// 本番サイトのページなら、次の起動で開き直す「パス?クエリ」を返す。覚えないページは None
fn resumable(url: &Url) -> Option<String> {
  if !matches!(url.scheme(), "http" | "https") || !is_base(url) {
    return None;
  }
  let path = url.path();
  let ok = RESUME_EXACT.contains(&path)
    || RESUME_PREFIX.iter().any(|p| path == *p || path.strip_prefix(p).is_some_and(|rest| rest.starts_with('/')));
  if !ok || path.len() > 300 {
    return None;
  }
  let query: Vec<(String, String)> = url
    .query_pairs()
    .filter(|(k, v)| RESUME_QUERY.contains(&k.as_ref()) && v.len() <= 100)
    .map(|(k, v)| (k.into_owned(), v.into_owned()))
    .collect();
  let mut out = path.to_string();
  if !query.is_empty() {
    out.push('?');
    out.push_str(&url::form_urlencoded::Serializer::new(String::new()).extend_pairs(query).finish());
  }
  // 初期化スクリプトの文字列に入れるので、URL として安全な文字だけ（Url が % で符号化した後の形）
  out
    .chars()
    .all(|c| c.is_ascii_alphanumeric() || "/_-.~%?=&+".contains(c))
    .then_some(out)
}

/// ログイン画面（ログアウト・ログイン切れ）に来たら、覚えたページを忘れる（次に使う人に前の人の画面を開かせない）
fn is_login_page(url: &Url) -> bool {
  is_base(url) && matches!(url.path(), "/login" | "/portal/login")
}

fn last_page_path<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
  app.path().app_data_dir().ok().map(|d| d.join(LAST_PAGE_FILE))
}

/// 前回の最後のページ（本番サイトの URL）。無い・読めない・覚えてよい形でないときは None
fn resume_url<R: Runtime>(app: &AppHandle<R>) -> Option<Url> {
  let saved = std::fs::read_to_string(last_page_path(app)?).ok()?;
  let url = BASE.join(saved.trim()).ok()?;
  resumable(&url).and_then(|p| BASE.join(&p).ok())
}

/// main の窓でページを読み終えたら、覚える・忘れる
fn remember_page<R: Runtime>(app: &AppHandle<R>, url: &Url) {
  let Some(file) = last_page_path(app) else { return };
  if let Some(p) = resumable(url) {
    if let Some(dir) = file.parent() {
      let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&file, p) {
      log::warn!("最後のページを覚えられませんでした: {e}");
    }
  } else if is_login_page(url) {
    let _ = std::fs::remove_file(&file);
  }
}

/// 同じオリジンの target=_blank（領収書・契約書 PDF・「新しいタブで開く」）はログイン Cookie が要るので、
/// 既定ブラウザではなくアプリ内の別窓で開く。別窓は 1 枚だけで、開いていれば使い回す
fn open_in_viewer(app: &AppHandle, url: Url) {
  if let Some(win) = app.get_webview_window(VIEWER) {
    navigate(&win, url);
    return;
  }
  let app = app.clone();
  // Windows は新規窓のハンドラの中で窓を作ると固まるので、別スレッドから作る
  std::thread::spawn(move || {
    if let Err(e) = build_window(&app, VIEWER, WebviewUrl::External(url)) {
      log::warn!("別窓を開けませんでした: {e}");
    }
  });
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
  let main = app.get_webview_window("main");
  // 操作の対象は手前の窓（別窓でファイルを見ているときはそちら）
  let focused = app.webview_windows().into_values().find(|w| w.is_focused().unwrap_or(false));
  let Some(win) = focused.or_else(|| main.clone()) else { return };
  match event.id().as_ref() {
    "home" => {
      let win = main.unwrap_or(win);
      navigate(&win, start_url());
      let _ = win.set_focus();
    }
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
      // 本番サイトのページならその URL、ローカル画面・ログイン途中・URL が取れないときは入口を開く
      let target = win
        .url()
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https") && is_base(u))
        .unwrap_or_else(start_url);
      open_external(app, &target);
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

/// 窓を作る。main も別窓も同じハンドラ（移動先の制限・新規窓・ダウンロード）を付ける
fn build_window(app: &AppHandle, label: &str, url: WebviewUrl) -> tauri::Result<WebviewWindow> {
  let version = app.package_info().version.to_string();
  let platform = if cfg!(target_os = "macos") { "macos" } else if cfg!(target_os = "windows") { "windows" } else { "linux" };
  // 起動時の画面（ui/main.js）は、接続を確かめたあと resumeUrl（前回の最後のページ）があればそこへ、無ければ入口へ移る
  let resume = if label == "main" { resume_url(app).map(|u| u.to_string()).unwrap_or_default() } else { String::new() };
  // titlebar: "overlay" のとき、Web 側は帯の左に信号ボタンの分を空け、帯をドラッグできるようにする（1.0.3 までは付かない）
  let titlebar = if OVERLAY_TITLEBAR && label == "main" { "overlay" } else { "native" };
  let init_script = format!(
    r#"window.__REPLICA_DESKTOP__ = {{ platform: "{platform}", version: "{version}", titlebar: "{titlebar}", startUrl: "{base}/portal", resumeUrl: "{resume}" }};
(function () {{ var f = function () {{ document.documentElement && document.documentElement.classList.add("replica-desktop"); }}; if (document.documentElement) f(); else document.addEventListener("DOMContentLoaded", f); }})();"#,
    base = BASE_URL.trim_end_matches('/'),
  );

  let nav_handle = app.clone();
  let new_window_handle = app.clone();
  let dl_handle = app.clone();

  let mut builder = WebviewWindowBuilder::new(app, label, url)
    .title("Replica Works")
    .min_inner_size(900.0, 600.0)
    .center()
    .user_agent(&user_agent(&version))
    .initialization_script(init_script)
    // Tauri のドラッグ＆ドロップ処理を切って、ページ側（HTML5）にファイルのドロップを届ける
    .disable_drag_drop_handler()
    .on_navigation(move |url| {
      if stays_inside(url) {
        true
      } else {
        open_external(&nav_handle, url);
        false
      }
    })
    .on_new_window(move |url, _features| {
      // target=_blank / window.open。同じオリジンはアプリ内の別窓、それ以外は既定ブラウザへ（about:blank などは無視）
      if matches!(url.scheme(), "http" | "https") && is_base(&url) {
        open_in_viewer(&new_window_handle, url);
      } else {
        open_external(&new_window_handle, &url);
      }
      NewWindowResponse::Deny
    })
    .on_download(move |_webview, event| {
      match event {
        DownloadEvent::Requested { url, destination } => {
          // 保存するのは本番サイトのファイルだけ（blob: も本番サイトで作られたものだけ）
          if !is_base(&url) {
            log::warn!("本番サイト以外からのダウンロードを止めました（{}:）", url.scheme());
            return false;
          }
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
    });

  builder = if label == VIEWER {
    // 別窓は中身が表示できると分かってから出す（「保存」で返るファイルは窓を出さずにダウンロードだけ行う）
    builder.inner_size(1000.0, 800.0).visible(false).on_page_load(|win, payload| {
      if matches!(payload.event(), PageLoadEvent::Started) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
      }
    })
  } else {
    let page_handle = app.clone();
    overlay_titlebar(builder.inner_size(1280.0, 860.0)).on_page_load(move |_win, payload| {
      if matches!(payload.event(), PageLoadEvent::Finished) {
        remember_page(&page_handle, payload.url());
      }
    })
  };

  builder.build()
}

/// Mac: タイトルバーを透明にして中身を窓の一番上から描く。信号ボタンは高さ 48px の帯の縦の真ん中に置く
#[cfg(target_os = "macos")]
fn overlay_titlebar<'a, R: Runtime, M: Manager<R>>(b: WebviewWindowBuilder<'a, R, M>) -> WebviewWindowBuilder<'a, R, M> {
  b.title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true)
    .traffic_light_position(tauri::LogicalPosition::new(18.0, 24.0))
}
#[cfg(not(target_os = "macos"))]
fn overlay_titlebar<'a, R: Runtime, M: Manager<R>>(b: WebviewWindowBuilder<'a, R, M>) -> WebviewWindowBuilder<'a, R, M> {
  b
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_opener::init())
    // 別窓は毎回同じ大きさ・非表示で作るので、位置と大きさの記憶から外す
    .plugin(tauri_plugin_window_state::Builder::default().with_denylist(&[VIEWER]).build())
    .manage(Downloads(Mutex::new(HashMap::new())))
    .manage(Zoom(Mutex::new(1.0)))
    .on_window_event(|window, event| {
      // 画面の中の移動（Next.js のリンク）は読み込みの合図が出ないので、閉じるときに今のページを覚える
      if window.label() == "main" && matches!(event, WindowEvent::CloseRequested { .. }) {
        remember_current(window.app_handle());
      }
      // main を閉じたら別窓（非表示のままのこともある）も閉じて、アプリを終了できるようにする
      if window.label() == "main"
        && matches!(event, WindowEvent::Destroyed)
        && let Some(viewer) = window.app_handle().get_webview_window(VIEWER)
      {
        let _ = viewer.close();
      }
    })
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // 最初はローカル画面（ui/index.html）。本番サイトに届くと分かってから入口（お客様ページのログイン）へ移り、
      // 届かなければ案内と再試行を出す
      if OVERLAY_TITLEBAR {
        let mut cap = tauri::ipc::CapabilityBuilder::new("overlay-drag").remote(format!("{}/*", BASE_URL.trim_end_matches('/'))).window("main");
        for p in DRAG_PERMISSIONS {
          cap = cap.permission(p);
        }
        app.add_capability(cap)?;
      }
      build_window(app.handle(), "main", WebviewUrl::App("index.html".into()))?;

      let menu = build_menu(app.handle())?;
      app.set_menu(menu)?;
      app.on_menu_event(on_menu);
      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run(|app, event| {
      // Mac の ⌘Q など、窓を閉じずに終えるとき
      if matches!(event, RunEvent::ExitRequested { .. }) {
        remember_current(app);
      }
    });
}

/// main の窓の今の URL を覚える（画面の中の移動のあとでも、WebView の URL は今のページを指す）
fn remember_current<R: Runtime>(app: &AppHandle<R>) {
  if let Some(url) = app.get_webview_window("main").and_then(|w| w.url().ok()) {
    remember_page(app, &url);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn u(s: &str) -> Url {
    Url::parse(s).unwrap()
  }

  // 既定の BASE_URL（本番）でビルドしたときの決まり。REPLICA_WORKS_BASE_URL を付けたビルドでは走らせない
  const PROD: &str = "https://replica-works--replica-d2b34.asia-east1.hosted.app";

  #[test]
  fn inside() {
    if BASE_URL != PROD {
      return;
    }
    for ok in [
      "https://replica-works--replica-d2b34.asia-east1.hosted.app/portal",
      "https://replica-works--replica-d2b34.asia-east1.hosted.app/__/auth/handler?x=1",
      "blob:https://replica-works--replica-d2b34.asia-east1.hosted.app/0b2f",
      "https://accounts.google.com/o/oauth2/auth",
      "https://accounts.youtube.com/accounts/SetSID",
      "tauri://localhost/index.html",
      "http://tauri.localhost/index.html",
      "about:blank",
    ] {
      assert!(stays_inside(&u(ok)), "{ok}");
    }
    for ng in [
      "http://replica-works--replica-d2b34.asia-east1.hosted.app/portal",
      "https://replica-works--replica-d2b34.asia-east1.hosted.app.example.com/",
      "https://evil.web.app/",
      "https://evil.firebaseapp.com/",
      "https://replica-d2b34.firebaseapp.com/__/auth/handler",
      "https://storage.googleapis.com/x",
      "https://www.gstatic.com/x",
      "https://notgoogle.com/",
      "https://evilgoogle.com/",
      "http://accounts.google.com/",
      "blob:https://example.com/0b2f",
      "data:text/html,<p>x",
      "file:///etc/passwd",
    ] {
      assert!(!stays_inside(&u(ng)), "{ng}");
    }
    // http の localhost はデバッグビルドだけ
    assert_eq!(stays_inside(&u("http://localhost:3000/")), cfg!(debug_assertions));
    assert_eq!(stays_inside(&u("http://127.0.0.1:9299/emulator/auth/handler")), cfg!(debug_assertions));
  }

  #[test]
  fn resume() {
    if BASE_URL != PROD {
      return;
    }
    let b = |p: &str| u(&format!("{PROD}{p}"));
    for (from, want) in [
      ("/", "/"),
      ("/portal", "/portal"),
      ("/tasks", "/tasks"),
      ("/projects/abc?tab=chat", "/projects/abc?tab=chat"),
      ("/projects/abc?tab=chat&token=x#top", "/projects/abc?tab=chat"),
      ("/customers?q=%E7%94%B0%E4%B8%AD", "/customers?q=%E7%94%B0%E4%B8%AD"),
      ("/accounting/expenses?m=2026-11", "/accounting/expenses?m=2026-11"),
      ("/portal/documents", "/portal/documents"),
    ] {
      assert_eq!(resumable(&b(from)).as_deref(), Some(want), "{from}");
    }
    for ng in [
      "/sign/tok123", "/portal/confirm/tok", "/portal/join/CODE", "/portal/login", "/login", "/api/files/1",
      "/download/x", "/portal/enter", "/portal/switch", "/portal/projects/abc", "/oauth/authorize", "/tasksx", "/__/auth/handler",
    ] {
      assert!(resumable(&b(ng)).is_none(), "{ng}");
    }
    assert!(resumable(&u("https://example.com/tasks")).is_none());
    assert!(resumable(&u("tauri://localhost/index.html")).is_none());
    assert!(is_login_page(&b("/portal/login?auto=0")) && is_login_page(&b("/login")) && !is_login_page(&b("/tasks")));
  }

  #[test]
  fn external_and_download() {
    for ok in ["http", "https", "mailto", "tel"] {
      assert!(is_external_scheme(ok));
    }
    for ng in ["file", "smb", "shell", "ms-settings", "about", "blob", "data", "javascript", "tauri"] {
      assert!(!is_external_scheme(ng));
    }
    if BASE_URL != PROD {
      return;
    }
    assert!(is_base(&u("https://replica-works--replica-d2b34.asia-east1.hosted.app/api/files/1")));
    assert!(is_base(&u("blob:https://replica-works--replica-d2b34.asia-east1.hosted.app/0b2f")));
    assert!(!is_base(&u("https://storage.googleapis.com/bucket/file.exe")));
    assert!(!is_base(&u("blob:https://accounts.google.com/0b2f")));
    assert!(!is_base(&u("data:application/octet-stream;base64,AAAA")));
  }
}
