# Replica Works デスクトップ版（Mac / Windows）

[Replica Works](https://github.com/replicamanabe-hue/replica-works)（社内管理画面）と、お客様ページ（`/portal`）を
専用の窓で開く [Tauri 2](https://tauri.app/) 製のアプリ。中身は本番 URL
`https://replica-works--replica-d2b34.asia-east1.hosted.app` をそのまま表示する薄いシェルで、
Web 側を直せばアプリ側も自動で新しくなる。

## できること

- 起動時に「社内管理画面」「お客様ページ」を選ぶ（記憶させて次回から直接開ける）
- メニュー「表示」で切り替え（⌘1 / Ctrl+1 社内、⌘2 / Ctrl+2 お客様、⌘0 / Ctrl+0 切替画面）、戻る・進む・再読み込み・拡大縮小
- 外部サイト、PDF（別窓で開くリンク）、`mailto:` などは OS の既定ブラウザ・アプリで開く
- ファイルのダウンロードは OS の「ダウンロード」フォルダへ保存し、保存後にフォルダで表示する
- 窓の位置・大きさを記憶する

Web 側（replica-works）はデスクトップ版を UA の `ReplicaWorksDesktop/<版>` か
`window.__REPLICA_DESKTOP__` で見分け、Google ログインをポップアップではなくリダイレクト式にする。

## 配布

タグを push すると GitHub Actions（`.github/workflows/release.yml`）が Mac（Universal `.dmg`）と
Windows（`-setup.exe` と `.msi`）を作り、GitHub Release に載せる。

```bash
# 版を上げる: package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml の version をそろえてから
git tag v1.0.0 && git push origin v1.0.0
```

ダウンロード: https://github.com/replicamanabe-hue/replica-works-desktop/releases/latest
（管理画面「管理者」ページにも同じリンクがある）

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

## Google ログインのための設定（1 回だけ）

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

- `ui/index.html` — 切替画面（ローカル）。選択を `localStorage` に記憶し、本番 URL へ移動する
- `src-tauri/src/lib.rs` — 窓・メニュー・外部リンク・ダウンロードの扱い。本番 URL は `BASE_URL`
- `src-tauri/tauri.conf.json` — アプリ名・識別子 `com.replicallc.replicaworks.desktop`・バンドル設定
- `src-tauri/capabilities/default.json` — 切替画面に渡す権限（最小）。本番 URL には Tauri API を渡さない
