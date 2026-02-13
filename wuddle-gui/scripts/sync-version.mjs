import fs from "node:fs";
import path from "node:path";

function parseVersion(input) {
  const raw = String(input || "").trim();
  if (!raw) return "";
  return raw.startsWith("v") ? raw.slice(1) : raw;
}

function isSemver(value) {
  return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(value);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
}

function updateCargoVersion(filePath, version) {
  const text = fs.readFileSync(filePath, "utf8");
  const packageVersionPattern = /^version\s*=\s*"([^"]*)"/m;
  const match = text.match(packageVersionPattern);
  if (!match) {
    throw new Error(`Could not update version in ${filePath}`);
  }
  if (match[1] === version) {
    return;
  }
  const next = text.replace(packageVersionPattern, `version = "${version}"`);
  fs.writeFileSync(filePath, next, "utf8");
}

const argVersion = process.argv[2] || process.env.WUDDLE_VERSION || process.env.GITHUB_REF_NAME;
const version = parseVersion(argVersion);

if (!version) {
  throw new Error("No version provided. Pass e.g. v1.0.2 or 1.0.2");
}
if (!isSemver(version)) {
  throw new Error(`Invalid semver: ${version}`);
}

const root = process.cwd();
const packageJsonPath = path.join(root, "package.json");
const tauriConfPath = path.join(root, "src-tauri", "tauri.conf.json");
const cargoTomlPath = path.join(root, "src-tauri", "Cargo.toml");

const pkg = readJson(packageJsonPath);
pkg.version = version;
writeJson(packageJsonPath, pkg);

const tauriConf = readJson(tauriConfPath);
tauriConf.version = version;
writeJson(tauriConfPath, tauriConf);

updateCargoVersion(cargoTomlPath, version);

console.log(`wuddle: synced app version to ${version}`);
