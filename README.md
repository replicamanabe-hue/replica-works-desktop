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

直接ダウンロードの Windows 版は当面未署名（初回に SmartScreen の「詳細情報」→「実行」が必要）。ストア版（下記）にはこの手順はない。

## 公式ストアでの配布（Mac App Store / Microsoft Store）

ユーザーへの本来の入口はストア。直接ダウンロードは審査待ちやストアを使えない環境向けの保険として残す。
ストア版はストアが署名・配信・更新を行うので、未署名の警告も自前の自動更新も要らない。
審査員向けには replica-works の `REVIEW_EMAIL` / `REVIEW_CODE`（固定コードで `/portal` に入れるデモログイン）を審査メモに書く。

### Mac App Store

バンドル ID は iOS 版と同じ `com.replicallc.replicaworks` にしてあり、App Store Connect の既存アプリ「Replica Works」に
macOS を **プラットフォーム追加** する形で載せる（掲載が 1 つにまとまる）。

1 回だけの準備（Apple Developer、Team `LD6XFR33N5`）:

1. 証明書を 2 枚作ってこの Mac のキーチェーンに入れる（developer.apple.com → Certificates）
   - **Apple Distribution**（アプリ本体の署名。iOS 版で作ったものがあればそれで良い）
   - **Mac Installer Distribution**（`.pkg` の署名。キーチェーンでは `3rd Party Mac Developer Installer: …` と出る）
2. Profiles で種類 **Mac App Store Connect** のプロファイルを App ID `com.replicallc.replicaworks` に対して作り、
   `src-tauri/appstore/ReplicaWorks.provisionprofile` として保存する（この場所は git に入らない）
3. App Store Connect → アプリ「Replica Works」→「プラットフォームを追加」→ macOS。スクリーンショットや説明文を入れる
4. （アップロードを自動化するなら）App Store Connect → ユーザとアクセス → 統合 → チーム キー で Developer 権限のキーを作り、
   `AuthKey_<KEY_ID>.p8` を `~/.appstoreconnect/private_keys/` に置く

提出のたびに:

```bash
# 版を上げてから（直接配布と同じ version を使う）
APPLE_SIGNING_IDENTITY="Apple Distribution: 合同会社Replica (LD6XFR33N5)" \
APPLE_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: 合同会社Replica (LD6XFR33N5)" \
APPLE_API_KEY_ID=XXXXXXXXXX APPLE_API_ISSUER=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx \
scripts/appstore-mac.sh
```

`dist/Replica Works_<版>_appstore.pkg` ができ、API キーがあればそのまま App Store Connect に上がる（無ければ Transporter.app で上げる）。
同じ版を直して再提出するときは `BUILD_NUMBER=2` のように増やす。
証明書名はキーチェーンアクセスか `security find-identity -v -p codesigning` で確認する。

App Store 向けビルドだけ `src-tauri/tauri.appstore.conf.json` を重ねて、App Sandbox の entitlements
（`src-tauri/Entitlements.appstore.plist`: ネットワーク接続とダウンロードフォルダへの書き込み）と Provisioning Profile を埋め込む。
`src-tauri/Info.plist` は暗号輸出の申告（HTTPS のみなので免除）で、どのビルドにも入る。

### Microsoft Store

Microsoft Store は exe/msi をそのまま出すと有料のコード署名証明書が要るので、**MSIX** で提出する
（Store が無料で署名し、更新も配信する）。

1 回だけの準備:

1. [Partner Center](https://partner.microsoft.com/dashboard) に **会社アカウント**で登録する（登録料は無料）。
   自社ドメインのメール（例: manabe@replica-llc.com）と、登記簿謄本など会社の証明書類か DUNS 番号が要り、審査に 2〜5 営業日かかる
2. 「新しい製品」→ **MSIX または PWA アプリ** で名前「Replica Works」を予約する
3. 製品の「製品 ID」ページに出る値を GitHub リポジトリの **Variables**（Settings → Secrets and variables → Actions → Variables）に入れる

| Variable | 中身（Partner Center の表記） |
| --- | --- |
| `MSIX_IDENTITY_NAME` | Package/Identity/Name（例 `12345ReplicaLLC.ReplicaWorks`） |
| `MSIX_PUBLISHER` | Package/Identity/Publisher（`CN=…` の形） |
| `MSIX_PUBLISHER_DISPLAY_NAME` | 発行者表示名（省略時 `合同会社Replica`） |

以後はタグを push するだけで Actions の Windows ジョブが `Replica Works_<版>_x64.msix` を作り GitHub Release に添付する
（`scripts/make-msix.ps1`。マニフェストは `src-tauri/msix/AppxManifest.xml`）。
Partner Center の提出でその `.msix` をアップロードし、審査に出す。`runFullTrust` 機能の理由を聞かれたら
「デスクトップ（Win32）アプリのため」と書く。

- WebView2 ランタイムは Windows 11 に標準で入っている。Windows 10 で無い場合はアプリが初回にインストールを案内する
- 版は `package.json` の `X.Y.Z` に `.0` を足した 4 桁になる（Store の規則）
- ストア掲載 URL が決まったら replica-works の `apphosting.yaml` に `MAC_APP_STORE_URL` / `MICROSOFT_STORE_URL` を入れると、
  管理者ページにストアのボタンが出て `/download/mac-store`・`/download/windows-store` が転送するようになる

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
