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
- 別窓で開くリンクのうち、同じサイトのファイル（領収書・契約書 PDF・「新しいタブで開く」）はログインが要るのでアプリ内の別窓で開く
- 外部サイト（http / https）と `mailto:` / `tel:` は OS の既定ブラウザ・アプリで開く。それ以外の種類の URL は開かない
- ファイルのダウンロードは本番サイトのものだけ受け付け、OS の「ダウンロード」フォルダへ保存して、保存後にフォルダで表示する
- 起動時に接続を確かめ、つながらないときは案内と「再試行」を出す（オンラインに戻ると自動で再試行）
- 窓の位置・大きさを記憶する

Web 側（replica-works）はデスクトップ版を UA の `ReplicaWorksDesktop/<版>` か
`window.__REPLICA_DESKTOP__` で見分け、`/login` の Google ログインをポップアップではなくリダイレクト式にする
（通常の入口はお客様ページなので使う場面は少ない）。

## 配布

**配布はストアだけ**（Mac App Store / Microsoft Store。2026-11-21 の判断で、直接ダウンロードの `.dmg` / `.exe` はやめた）。
前に配った `/download/mac`・`/download/windows`・`/download/windows-msi` は、ストアの掲載 URL を設定するとストアへ転送される。

1. タグを push すると GitHub Actions（`.github/workflows/release.yml`）が Windows をビルドし、
   Microsoft Store に出す `.msix` だけを GitHub Release に載せる（インストーラーは作らない）。タグと 3 つの version が食い違っていると最初に止まる。
2. Mac App Store の `.pkg` は手元の Mac で `scripts/appstore-mac.sh`（下記）。

```bash
# 版を上げる: package.json / src-tauri/tauri.conf.json / src-tauri/Cargo.toml の version をそろえてから
git tag v1.0.4 && git push origin v1.0.4
```

Windows の Actions ジョブが 504 で落ちたら `gh run rerun <run-id> --failed` で再実行する（それまで Release は下書きのまま）。

### Mac の署名・公証（任意だが推奨）

未署名のままだと初回起動時に「開発元を確認できません」と出る（一度開こうとしたあと、システム設定 → プライバシーとセキュリティ →
「このまま開く」で起動できる。macOS 15 以降は右クリック→「開く」では通らない）。
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

直接ダウンロードはやめたので、この署名・公証は使っていない（ストア版はストアが署名する）。

## 公式ストアでの配布（Mac App Store / Microsoft Store）

ユーザーへの入口はストアだけ。ストア版はストアが署名・配信・更新を行うので、未署名の警告も自前の自動更新も要らない。
審査員向けには replica-works の `REVIEW_EMAIL` / `REVIEW_CODE`（固定コードで `/portal` に入れるデモログイン）を審査メモに書く。

### Mac App Store

バンドル ID は iOS 版と同じ `com.replicallc.replicaworks` にしてあり、App Store Connect の既存アプリ「Replica Works」に
macOS を **プラットフォーム追加** する形で載せる（掲載が 1 つにまとまる）。

1 回だけの準備（Apple Developer、Team `LD6XFR33N5`）。手作業は API キーを作るところだけで、残りは `scripts/apple-setup.mjs` が行う:

1. App Store Connect → ユーザとアクセス → 統合 → **チームキー** で、役割 **Admin** のキーを作る（証明書の発行に Admin が要る）。
   Issuer ID とキー ID を控え、落ちてきた `AuthKey_<キーID>.p8` を `~/.appstoreconnect/private_keys/` に置く
2. 実行する

   ```bash
   APPLE_API_KEY_ID=<キーID> APPLE_API_ISSUER=<Issuer ID> node scripts/apple-setup.mjs
   ```

   署名鍵と CSR を `~/.appstoreconnect/replica-works-mac/` に作り、証明書 **Apple Distribution** と **Mac Installer Distribution** を発行して
   キーチェーンに入れ、Provisioning Profile（種類 Mac App Store Connect、App ID `com.replicallc.replicaworks`）を
   `src-tauri/appstore/ReplicaWorks.provisionprofile` に保存し、App Store Connect の既存アプリに macOS の版を追加する。
   何度実行しても同じ結果になる（あるものは使い、無いものだけ作る）
3. App Store Connect → アプリ「Replica Works」→ macOS の版で、スクリーンショット・説明文・カテゴリ（ビジネス）・年齢区分・
   「App Review に関する情報」（デモログイン: replica-works の `REVIEW_EMAIL` / `REVIEW_CODE`）を入れる

手で行う場合は developer.apple.com の Certificates で上記 2 種類の証明書を CSR `~/.appstoreconnect/replica-works-mac/request.csr` から作り、
Profiles で種類 **Mac App Store Connect** のプロファイルを作って同じ場所に置く。

提出のたびに:

```bash
# 版を上げてから（直接配布と同じ version を使う）
APPLE_SIGNING_IDENTITY="Apple Distribution: 合同会社Replica (LD6XFR33N5)" \
APPLE_INSTALLER_IDENTITY="3rd Party Mac Developer Installer: 合同会社Replica (LD6XFR33N5)" \
APPLE_API_KEY_ID=XXXXXXXXXX APPLE_API_ISSUER=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx \
scripts/appstore-mac.sh
```

`dist/Replica Works_<版>_appstore.pkg` ができ、API キーがあればそのまま App Store Connect に上がる（無ければ Transporter.app で上げる）。
Apple は「Intel 対応（Universal）か、最低 macOS 12 以上」を要求するので Universal で作る。Homebrew の `rust` には Intel 用の標準ライブラリが無いため、
`brew install rustup`（keg-only で既存の rust と衝突しない）を入れておくと、スクリプトがそのツールチェーンで両アーキテクチャをビルドする。
同じ版を直して再提出するときは `BUILD_NUMBER=2` のように増やす。
証明書名はキーチェーンアクセスか `security find-identity -v -p codesigning` で確認する。

App Store 向けビルドだけ `src-tauri/tauri.appstore.conf.json` を重ねて、App Sandbox の entitlements
（`src-tauri/Entitlements.appstore.plist`: ネットワーク接続、ダウンロードフォルダへの書き込み、ファイル選択で選んだファイルの読み書き）と
Provisioning Profile を埋め込む。
`src-tauri/Info.plist` は暗号輸出の申告（HTTPS のみなので免除）と既定の言語（日本語）で、どのビルドにも入る。

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

- WebView2 ランタイムは Windows 11 に標準で入っている。MSIX には WebView2 のインストーラーが入らないので、Windows 10 で無い場合は
  利用者が Microsoft の配布ページから入れる
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

アプリの窓の中で開けるのは本番サイトと、ログイン途中の Google（`*.google.com` / `*.youtube.com` / `accounts.google.co.jp`）だけ
（`src-tauri/src/lib.rs` の `stays_inside`）。それ以外へ移動しようとすると既定ブラウザで開く。

## 開発

```bash
npm install
npm run dev                                  # 本番 URL を表示
REPLICA_WORKS_BASE_URL=http://localhost:3000 npm run dev   # ローカルの replica-works を表示
npm run build                                # src-tauri/target/release/bundle/ に .app / .dmg
npm run icon                                 # app-icon.png からアイコン一式を作り直す
```

`REPLICA_WORKS_BASE_URL` を付けて配布用にビルドする場合は、`src-tauri/tauri.conf.json` の `csp`（`connect-src`）も同じホストに直す。

要件: Node 22+、Rust（`brew install rust`）、Xcode Command Line Tools。Windows 版の生成は GitHub Actions に任せる。

## 構成

- `ui/` — 起動時に出るローカル画面（`index.html` / `main.js` / `style.css`）。本番サイトの `/api/health` に届けば入口へ移り、
  届かなければ案内と再試行を出す。Tauri API は使わない
- `src-tauri/src/lib.rs` — 窓・メニュー・外部リンク・ダウンロードの扱い。本番 URL は `BASE_URL`、入口は `start_url()`（`/portal`）
- `src-tauri/tauri.conf.json` — アプリ名・識別子 `com.replicallc.replicaworks`・ローカル画面の CSP・バンドル設定
- `src-tauri/capabilities/default.json` — ローカル画面に渡す権限（空）。本番 URL にも別窓にも Tauri API を渡さない
