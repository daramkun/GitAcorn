import { readFile } from "node:fs/promises";

const rootPackage = JSON.parse(await readFile("package.json", "utf8"));
const desktopPackage = JSON.parse(
  await readFile("apps/desktop/package.json", "utf8"),
);
const tauriConfig = JSON.parse(
  await readFile("apps/desktop/src-tauri/tauri.conf.json", "utf8"),
);
const cargoManifest = await readFile("Cargo.toml", "utf8");
const cargoVersion = cargoManifest.match(
  /\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/,
)?.[1];

if (!cargoVersion) {
  throw new Error("Cargo.toml의 workspace.package.version을 찾지 못했습니다.");
}

const versions = new Map([
  ["package.json", rootPackage.version],
  ["apps/desktop/package.json", desktopPackage.version],
  ["Cargo.toml", cargoVersion],
  ["apps/desktop/src-tauri/tauri.conf.json", tauriConfig.version],
]);
const uniqueVersions = new Set(versions.values());

if (uniqueVersions.size !== 1) {
  const details = [...versions]
    .map(([file, version]) => `  ${file}: ${version}`)
    .join("\n");
  throw new Error(`프로젝트 버전이 일치하지 않습니다.\n${details}`);
}

const version = versions.values().next().value;
const semverPattern =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

if (!semverPattern.test(version)) {
  throw new Error(`프로젝트 버전 ${version}이 유효한 SemVer가 아닙니다.`);
}

const tagArgumentIndex = process.argv.indexOf("--tag");

if (tagArgumentIndex !== -1) {
  const tag = process.argv[tagArgumentIndex + 1];
  if (!tag) {
    throw new Error("--tag 다음에 v0.1.0 형식의 태그가 필요합니다.");
  }
  if (!tag.startsWith("v") || !semverPattern.test(tag.slice(1))) {
    throw new Error(`릴리스 태그 ${tag}가 v 접두사를 포함한 유효한 SemVer가 아닙니다.`);
  }
  if (tag !== `v${version}`) {
    throw new Error(`릴리스 태그 ${tag}와 프로젝트 버전 v${version}이 다릅니다.`);
  }
}

console.log(`GitAcorn version ${version}`);
