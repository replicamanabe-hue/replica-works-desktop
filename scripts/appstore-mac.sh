#!/usr/bin/env bash
# Mac App Store 提出用の .pkg を作り、環境変数があれば App Store Connect にアップロードする。
# 手元の Mac（Xcode 入り）で実行する。証明書はキーチェーンに入っていること。
#
#   APPLE_SIGNING_IDENTITY   必須。アプリ署名。例: "Apple Distribution: 合同会社Replica (LD6XFR33N5)"
#   APPLE_INSTALLER_IDENTITY 必須。pkg 署名。例: "3rd Party Mac Developer Installer: 合同会社Replica (LD6XFR33N5)"
#   BUILD_NUMBER             任意。同じ版を再提出するときに増やす（CFBundleVersion）
#   APPLE_API_KEY_ID / APPLE_API_ISSUER  任意。両方あれば altool でアップロードまで行う
#                            （鍵 AuthKey_<KEY_ID>.p8 は ~/.appstoreconnect/private_keys に置く）
#   Provisioning Profile（種類: Mac App Store Connect）を src-tauri/appstore/ReplicaWorks.provisionprofile に置く
set -euo pipefail
cd "$(dirname "$0")/.."

: "${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY を設定してください（例: Apple Distribution: ... (LD6XFR33N5)）}"
: "${APPLE_INSTALLER_IDENTITY:?APPLE_INSTALLER_IDENTITY を設定してください（例: 3rd Party Mac Developer Installer: ... (LD6XFR33N5)）}"
PROFILE="src-tauri/appstore/ReplicaWorks.provisionprofile"
[ -f "$PROFILE" ] || { echo "Provisioning Profile がありません: $PROFILE" >&2; exit 1; }

VERSION=$(node -p "require('./package.json').version")
APP="src-tauri/target/universal-apple-darwin/release/bundle/macos/Replica Works.app"
OUT="dist/Replica Works_${VERSION}_appstore.pkg"
mkdir -p dist

rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null

# App Store 用の設定（Sandbox の entitlements と Provisioning Profile）を重ねてビルド
export APPLE_SIGNING_IDENTITY
npm run tauri -- build --no-bundle --target universal-apple-darwin
EXTRA=()
if [ -n "${BUILD_NUMBER:-}" ]; then
  EXTRA=(--config "{\"bundle\":{\"macOS\":{\"bundleVersion\":\"$BUILD_NUMBER\"}}}")
fi
npm run tauri -- bundle --bundles app --target universal-apple-darwin --config src-tauri/tauri.appstore.conf.json "${EXTRA[@]}"

echo "--- 署名と entitlements の確認"
codesign -dv --entitlements - "$APP" 2>&1 | grep -E "Authority=|app-sandbox|application-identifier" || true
[ -f "$APP/Contents/embedded.provisionprofile" ] || { echo "embedded.provisionprofile が入っていません" >&2; exit 1; }

echo "--- pkg を作成"
xcrun productbuild --sign "$APPLE_INSTALLER_IDENTITY" --component "$APP" /Applications "$OUT"
echo "作成: $OUT"

if [ -n "${APPLE_API_KEY_ID:-}" ] && [ -n "${APPLE_API_ISSUER:-}" ]; then
  echo "--- App Store Connect にアップロード"
  xcrun altool --upload-app --type macos --file "$OUT" --apiKey "$APPLE_API_KEY_ID" --apiIssuer "$APPLE_API_ISSUER"
else
  echo "APPLE_API_KEY_ID / APPLE_API_ISSUER が無いのでアップロードは省略。Transporter.app で $OUT を上げるか、環境変数を設定して再実行してください"
fi
