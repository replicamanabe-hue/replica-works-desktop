#!/usr/bin/env node
/**
 * Mac App Store 配布の Apple 側の準備を App Store Connect API で行う（手元の Mac で実行）。
 *
 *   1. 署名鍵と CSR（~/.appstoreconnect/replica-works-mac/）を用意する
 *   2. 証明書 Apple Distribution / Mac Installer Distribution を発行してキーチェーンに入れる
 *   3. Provisioning Profile（Mac App Store Connect）を作って src-tauri/appstore/ に置く
 *   4. App Store Connect の既存アプリ（iOS 版）に macOS の版を追加する
 *
 * 必要な環境変数（App Store Connect → ユーザとアクセス → 統合 → チームキー。役割は Admin）:
 *   APPLE_API_KEY_ID   キー ID
 *   APPLE_API_ISSUER   Issuer ID
 *   APPLE_API_KEY_PATH 任意。既定は ~/.appstoreconnect/private_keys/AuthKey_<キーID>.p8
 * 環境変数が無いときは 1 だけ行って、残りの手順を表示する。
 */
import { execFileSync } from "node:child_process";
import { createSign } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLE_ID = "com.replicallc.replicaworks";
const PROFILE_NAME = "Replica Works Mac App Store";
const PROFILE_OUT = path.join(ROOT, "src-tauri/appstore/ReplicaWorks.provisionprofile");
const WORK = path.join(os.homedir(), ".appstoreconnect/replica-works-mac");
const KEY_PEM = path.join(WORK, "key.pem");
const CSR = path.join(WORK, "request.csr");
const LOGIN_KEYCHAIN = path.join(os.homedir(), "Library/Keychains/login.keychain-db");
const API = "https://api.appstoreconnect.apple.com";

const sh = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], ...opts });
const log = (s) => console.log(s);

// ---- 1. 署名鍵と CSR
function ensureCsr() {
  fs.mkdirSync(WORK, { recursive: true, mode: 0o700 });
  if (!fs.existsSync(KEY_PEM)) {
    sh("openssl", ["genrsa", "-out", KEY_PEM, "2048"]);
    fs.chmodSync(KEY_PEM, 0o600);
    log(`署名鍵を作成: ${KEY_PEM}`);
  }
  if (!fs.existsSync(CSR)) {
    sh("openssl", ["req", "-new", "-key", KEY_PEM, "-out", CSR, "-subj", "/CN=Replica LLC/O=Replica LLC/C=JP"]);
    log(`CSR を作成: ${CSR}`);
  }
}

// ---- App Store Connect API
function jwt(keyId, issuer, pem) {
  const b64 = (o) => Buffer.from(typeof o === "string" ? o : JSON.stringify(o)).toString("base64url");
  const now = Math.floor(Date.now() / 1000);
  const data = `${b64({ alg: "ES256", kid: keyId, typ: "JWT" })}.${b64({ iss: issuer, iat: now, exp: now + 1200, aud: "appstoreconnect-v1" })}`;
  const sig = createSign("SHA256").update(data).sign({ key: pem, dsaEncoding: "ieee-p1363" });
  return `${data}.${sig.toString("base64url")}`;
}

function client() {
  const keyId = process.env.APPLE_API_KEY_ID;
  const issuer = process.env.APPLE_API_ISSUER;
  if (!keyId || !issuer) return null;
  const keyPath = process.env.APPLE_API_KEY_PATH ?? path.join(os.homedir(), `.appstoreconnect/private_keys/AuthKey_${keyId}.p8`);
  if (!fs.existsSync(keyPath)) throw new Error(`API キーのファイルがありません: ${keyPath}（App Store Connect で落とした .p8 をここに置く）`);
  const pem = fs.readFileSync(keyPath, "utf8");
  return async (method, p, body) => {
    const res = await fetch(`${API}${p}`, {
      method,
      headers: { Authorization: `Bearer ${jwt(keyId, issuer, pem)}`, "Content-Type": "application/json" },
      body: body ? JSON.stringify(body) : undefined,
    });
    const text = await res.text();
    const json = text ? JSON.parse(text) : {};
    if (!res.ok) {
      const detail = (json.errors ?? []).map((e) => `${e.code ?? e.status}: ${e.title} ${e.detail ?? ""}`).join("\n  ");
      const err = new Error(`${method} ${p} → ${res.status}\n  ${detail}`);
      err.status = res.status;
      err.codes = (json.errors ?? []).map((e) => e.code ?? "");
      throw err;
    }
    return json;
  };
}

// ---- キーチェーン
function identities() {
  const out = sh("security", ["find-identity", "-v"]);
  const pick = (re) => [...out.matchAll(re)].map((m) => m[1]);
  return {
    dist: pick(/"(Apple Distribution: [^"]+)"/g),
    installer: pick(/"(3rd Party Mac Developer Installer: [^"]+)"/g),
    untrusted: /CSSMERR_TP_NOT_TRUSTED/.test(out),
  };
}

function importKeyOnce() {
  try {
    sh("security", ["import", KEY_PEM, "-k", LOGIN_KEYCHAIN, "-T", "/usr/bin/codesign", "-T", "/usr/bin/productbuild", "-T", "/usr/bin/security"]);
    log("署名鍵をキーチェーンに登録");
  } catch (e) {
    if (!/already exists/.test(String(e.stderr ?? e))) throw e;
  }
}

function importCert(der, name) {
  const file = path.join(WORK, `${name}.cer`);
  fs.writeFileSync(file, der);
  try {
    sh("security", ["import", file, "-k", LOGIN_KEYCHAIN]);
  } catch (e) {
    if (!/already exists/.test(String(e.stderr ?? e))) throw e;
  }
  return file;
}

// Apple の中間証明書（WWDR G*）が無いと identity が信頼されないので、発行元に合わせて入れる
function ensureIntermediate(certFile) {
  if (!identities().untrusted) return;
  const issuer = sh("openssl", ["x509", "-inform", "der", "-in", certFile, "-noout", "-issuer"]);
  const g = issuer.match(/Authority\s*-?\s*(G\d)/i)?.[1] ?? "G3";
  const url = `https://www.apple.com/certificateauthority/AppleWWDRCA${g}.cer`;
  log(`中間証明書 ${g} を取得: ${url}`);
  return fetch(url).then(async (r) => {
    if (!r.ok) throw new Error(`中間証明書の取得に失敗: ${r.status}`);
    importCert(Buffer.from(await r.arrayBuffer()), `AppleWWDRCA${g}`);
  });
}

// キーチェーン内の証明書のシリアル番号（API の serialNumber と突き合わせる）
function keychainSerials(commonNamePrefix) {
  try {
    const pem = sh("security", ["find-certificate", "-a", "-c", commonNamePrefix, "-p", LOGIN_KEYCHAIN]);
    return pem.split("-----END CERTIFICATE-----").filter((s) => s.includes("BEGIN")).map((s) => {
      const serial = sh("openssl", ["x509", "-noout", "-serial"], { input: s + "-----END CERTIFICATE-----\n" });
      return serial.replace(/^serial=/, "").trim().toUpperCase();
    });
  } catch {
    return [];
  }
}

// ---- 2. 証明書
async function ensureCertificate(api, type, label, cnPrefix) {
  const list = await api("GET", `/v1/certificates?filter[certificateType]=${type}&limit=200`);
  const have = keychainSerials(cnPrefix);
  const existing = list.data.find((c) => have.includes(String(c.attributes.serialNumber).toUpperCase()));
  if (existing) {
    log(`${label}: キーチェーンにあるもの（${existing.attributes.name}）を使う`);
    return existing;
  }
  log(`${label}: 発行中…`);
  let created;
  try {
    created = await api("POST", "/v1/certificates", {
      data: { type: "certificates", attributes: { certificateType: type, csrContent: fs.readFileSync(CSR, "utf8") } },
    });
  } catch (e) {
    if (list.data.length) {
      throw new Error(
        `${label} の発行に失敗しました。既に ${list.data.length} 枚あり上限の可能性があります。` +
          `どれも秘密鍵がこの Mac に無いので、使っていないものを developer.apple.com で失効させてから再実行してください。\n` +
          list.data.map((c) => `  - ${c.attributes.name}（期限 ${c.attributes.expirationDate?.slice(0, 10)}）`).join("\n") +
          `\n元のエラー: ${e.message}`,
      );
    }
    throw e;
  }
  importKeyOnce();
  const file = importCert(Buffer.from(created.data.attributes.certificateContent, "base64"), type);
  await ensureIntermediate(file);
  log(`${label}: 発行してキーチェーンに登録（期限 ${created.data.attributes.expirationDate?.slice(0, 10)}）`);
  return created.data;
}

// ---- 3. Provisioning Profile
async function ensureProfile(api, distCertId) {
  const bundles = await api("GET", `/v1/bundleIds?filter[identifier]=${encodeURIComponent(BUNDLE_ID)}`);
  const bundle = bundles.data.find((b) => b.attributes.identifier === BUNDLE_ID);
  if (!bundle) throw new Error(`App ID ${BUNDLE_ID} が見つかりません（iOS 版の App ID と同じはず）`);
  log(`App ID: ${bundle.attributes.name}（platform ${bundle.attributes.platform}）`);

  const profiles = await api("GET", `/v1/profiles?filter[profileType]=MAC_APP_STORE&filter[name]=${encodeURIComponent(PROFILE_NAME)}&include=certificates`);
  let profile = profiles.data.find(
    (p) => p.attributes.profileState === "ACTIVE" && (p.relationships?.certificates?.data ?? []).some((c) => c.id === distCertId),
  );
  // 古い/証明書違いのものは消して作り直す
  for (const p of profiles.data) {
    if (p !== profile) {
      await api("DELETE", `/v1/profiles/${p.id}`);
      log(`古いプロファイルを削除: ${p.attributes.name}`);
    }
  }
  if (!profile) {
    log("Provisioning Profile を作成中…");
    const r = await api("POST", "/v1/profiles", {
      data: {
        type: "profiles",
        attributes: { name: PROFILE_NAME, profileType: "MAC_APP_STORE" },
        relationships: {
          bundleId: { data: { type: "bundleIds", id: bundle.id } },
          certificates: { data: [{ type: "certificates", id: distCertId }] },
        },
      },
    });
    profile = r.data;
  }
  fs.mkdirSync(path.dirname(PROFILE_OUT), { recursive: true });
  fs.writeFileSync(PROFILE_OUT, Buffer.from(profile.attributes.profileContent, "base64"));
  log(`Provisioning Profile を保存: ${path.relative(ROOT, PROFILE_OUT)}（期限 ${profile.attributes.expirationDate?.slice(0, 10)}）`);
}

// ---- 4. App Store Connect に macOS を追加
async function ensureMacVersion(api) {
  const version = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf8")).version;
  const apps = await api("GET", `/v1/apps?filter[bundleId]=${encodeURIComponent(BUNDLE_ID)}`);
  const app = apps.data[0];
  if (!app) throw new Error(`App Store Connect に ${BUNDLE_ID} のアプリがありません`);
  log(`App Store Connect: ${app.attributes.name}（${app.id}）`);
  const versions = await api("GET", `/v1/apps/${app.id}/appStoreVersions?filter[platform]=MAC_OS&limit=5`);
  if (versions.data.length) {
    const v = versions.data[0];
    log(`macOS の版はすでにあります: ${v.attributes.versionString}（${v.attributes.appStoreState ?? v.attributes.appVersionState}）`);
    return;
  }
  try {
    await api("POST", "/v1/appStoreVersions", {
      data: {
        type: "appStoreVersions",
        attributes: { platform: "MAC_OS", versionString: version },
        relationships: { app: { data: { type: "apps", id: app.id } } },
      },
    });
    log(`macOS の版 ${version} を追加しました。説明文・スクリーンショット・審査情報は App Store Connect で入れます`);
  } catch (e) {
    log(`macOS の版を API で追加できませんでした（${e.message.split("\n")[0]}）。App Store Connect のアプリ画面で「プラットフォームを追加」→ macOS を手で行ってください`);
  }
}

// ---- main
ensureCsr();
const api = client();
if (!api) {
  log(`
API キーが未設定なので、ここまで（署名鍵と CSR）で止めます。続けるには:
  1. https://appstoreconnect.apple.com/access/integrations/api で「チームキー」を作る（役割: Admin。証明書の発行に必要）
  2. 落ちてきた AuthKey_<キーID>.p8 を ~/.appstoreconnect/private_keys/ に置く
  3. APPLE_API_KEY_ID=<キーID> APPLE_API_ISSUER=<Issuer ID> node scripts/apple-setup.mjs
手で証明書を作る場合の CSR: ${CSR}`);
  process.exit(0);
}

const dist = await ensureCertificate(api, "DISTRIBUTION", "Apple Distribution", "Apple Distribution");
await ensureCertificate(api, "MAC_INSTALLER_DISTRIBUTION", "Mac Installer Distribution", "3rd Party Mac Developer Installer");
await ensureProfile(api, dist.id);
await ensureMacVersion(api);

const ids = identities();
log(`
完了。キーチェーンの署名 ID:
  アプリ:   ${ids.dist[0] ?? "(見つかりません)"}
  インストーラ: ${ids.installer[0] ?? "(見つかりません)"}${ids.untrusted ? "\n  注意: 信頼されていない証明書があります（security find-identity -v で確認）" : ""}

次は scripts/appstore-mac.sh（署名 ID は自動で拾う。APPLE_API_KEY_ID / APPLE_API_ISSUER があればアップロードまで行う）`);
