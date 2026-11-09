# Replica Works デスクトップ版（Mac / Windows）

[Replica Works](https://github.com/replicamanabe-hue/replica-works)（社内管理画面）と、お客様ページ（`/portal`）を
専用の窓で開く [Tauri 2](https://tauri.app/) 製のアプリ。中身は本番 URL
`https://replica-works--replica-d2b34.asia-east1.hosted.app` をそのまま表示する薄いシェルで、
Web 側を直せばアプリ側も自動で新しくなる。

## できること

- 起動するとお客様ページのログイン画面（メールの確認コード）。ストアのアプリと同じ入口
- 管理者のメールでログインした人だけ、右上メニューに「管理画面に切り替え」が出る（Web 側 `/portal/switch` が
  お客様ページのログインから管理画面のセッションを作る。管理者から外せば次の操作から入れなくなる）
- 管理画面の「お客様ページ」ボタンで戻れる
- メニュー「表示」: ホーム（⌘⇧H / Ctrl+Shift+H）、戻る・進む・再読み込み・拡大縮小・フルスクリーン
- 外部サイト、PDF（別窓で開くリンク）、`mailto:` などは OS の既定ブラウザ・アプリで開く
- ファイルのダウンロードは OS の「ダウンロード」フォルダへ保存し、保存後にフォルダで表示する
- 窓の位置・大きさを記憶する

Web 側（replica-works）はデスクトップ版を UA の `ReplicaWorksDesktop/<版>` か
`window.__REPLICA_DESKTOP__` で見分け、`/login` の Google ログインをポップアップではなくリダイレクト式にする
（通常の入口はお客様ページなので使う場面は少ない）。

## 配布

1. タグを push すると GitHub Actions（`.github/workflows/release.yml`）が Mac（Universal `.dmg`）と
   Windows（`-setup.exe` と `.msi`）を作り、GitHub Release に載せる。
2. replica-works 側で `node scripts/upload-desktop.mjs vX.Y.Z` を実行すると、その Release のファイルを
   Firebase Storage の `desktop/` にコピーし、`latest.json` を更新する（GitHub の Release 配信が不安定だったため、
   ユーザーへの配布は Storage から行う）。

```bash
# 版を上げる: package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml の version をそろえてから
git tag v1.0.2 && git push origin v1.0.2
# Actions が終わったら（replica-works で）
node scripts/upload-desktop.mjs v1.0.2
```

ダウンロード（誰でも可）:
- Mac: https://replica-works--replica-d2b34.asia-east1.hosted.app/download/mac
- Windows: https://replica-works--replica-d2b34.asia-east1.hosted.app/download/windows （MSI は `/download/windows-msi`）

管理画面「管理者」ページにも同じリンクと現在の版が出る。Windows の Actions ジョブが 504 で落ちたら
`gh run rerun <run-id> --failed` で再実行する。

### Mac の署名・公証（任意だが推奨）

未署名のままだと初回起動時に「開発元を確認できません」と出る（右クリック→「開く」で起動できる）。
Apple Developer（Team `LD6XFR33N5`）の **Developer ID Application** 証明書を作って、リポジトリの Secrets に入れると
Actions が署名と公証まで行う。

| Secret | 中身 |
| --- | --- |
| `APPLE_CERTIFICATE` | 証明書を書き出した `.p12` を `base64 -i cert.p12` した文字列 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` のパスワード |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: 合同会社Replica (LD6XFR33N5)` の形 |
| `APPLE_ID` | Apple ID のメールアドレス |
| `APPLE_PASSWORD` | その Apple ID の **App 用パスワード**（appleid.apple.com で発行） |
| `APPLE_TEAM_ID` | `LD6XFR33N5` |

Windows は当面未署名（初回に SmartScreen の「詳細情報」→「実行」が必要）。

## Google ログインのための設定（任意・1 回だけ）

通常はお客様ページのログインから管理画面へ切り替えるので不要。アプリ内で `/login` の Google ログインも使いたい場合だけ。
デスクトップ版はリダイレクト式でログインするため、Web 側は authDomain を自分のホストにし、
`/__/auth/*` を `replica-d2b34.firebaseapp.com` に中継している。これが動くには
**Google Cloud Console → API とサービス → 認証情報 → OAuth 2.0 クライアント ID「Web client (auto created by Google Service)」**
の「承認済みのリダイレクト URI」に次を追加する。

```
https://replica-works--replica-d2b34.asia-east1.hosted.app/__/auth/handler
```

（Firebase コンソール → Authentication → 設定 → 承認済みドメインに同じホストが入っていることも確認。）

## 開発

```bash
npm install
npm run dev                                  # 本番 URL を表示
REPLICA_WORKS_BASE_URL=http://localhost:3000 npm run dev   # ローカルの replica-works を表示
npm run build                                # src-tauri/target/release/bundle/ に .app / .dmg
npm run icon                                 # app-icon.png からアイコン一式を作り直す
```

要件: Node 22+、Rust（`brew install rust`）、Xcode Command Line Tools。Windows 版の生成は GitHub Actions に任せる。

## 構成

- `ui/index.html` — 接続できなかったときの案内（Tauri の `frontendDist` として必要。通常は表示されない）
- `src-tauri/src/lib.rs` — 窓・メニュー・外部リンク・ダウンロードの扱い。本番 URL は `BASE_URL`、入口は `start_url()`（`/portal`）
- `src-tauri/tauri.conf.json` — アプリ名・識別子 `com.replicallc.replicaworks.desktop`・バンドル設定
- `src-tauri/capabilities/default.json` — ローカル画面に渡す権限（最小）。本番 URL には Tauri API を渡さない
