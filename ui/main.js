// 起動時の画面。本番サイトに届くか確かめてから入口（お客様ページ）へ移る。届かなければ案内と再試行を出す。
// Tauri API は使わない（Mac の新しい版で、上の帯のドラッグで窓を動かす命令だけ許している）
(function () {
  var desktop = window.__REPLICA_DESKTOP__ || {};
  if (desktop.titlebar === "overlay") document.documentElement.classList.add("overlay");
  var startUrl = desktop.startUrl || "https://replica-works--replica-d2b34.asia-east1.hosted.app/portal";
  var healthUrl = new URL("/api/health", startUrl).href;
  // 前回の最後のページ（同じサイトのときだけ）。無ければ入口
  var target = startUrl;
  try {
    if (desktop.resumeUrl && new URL(desktop.resumeUrl).origin === new URL(startUrl).origin) target = desktop.resumeUrl;
  } catch (e) { /* 形が違えば入口 */ }
  var connecting = document.getElementById("connecting");
  var notice = document.getElementById("notice");
  var busy = false;

  function fail() {
    busy = false;
    connecting.hidden = true;
    notice.hidden = false;
  }

  function enter() {
    location.replace(target);
    // 移動できていればこの画面はもう無い。残っていたら（途中で切れたなど）案内に戻す
    setTimeout(fail, 20000);
  }

  function check() {
    if (busy) return;
    busy = true;
    notice.hidden = true;
    connecting.hidden = false;
    var ctrl = new AbortController();
    var timer = setTimeout(function () { ctrl.abort(); }, 15000);
    // no-cors なので中身は読めない。応答が返れば（状態コードによらず）サイトに届いている
    fetch(healthUrl, { mode: "no-cors", cache: "no-store", signal: ctrl.signal })
      .then(enter, fail)
      .finally(function () { clearTimeout(timer); });
  }

  // BASE_URL を差し替えたビルドで CSP の connect-src を合わせ忘れても、ここで止まらないようにそのまま入口へ進む
  document.addEventListener("securitypolicyviolation", function (e) {
    if (e.effectiveDirective === "connect-src") enter();
  });
  document.getElementById("retry").addEventListener("click", check);
  window.addEventListener("online", check);
  check();
})();
