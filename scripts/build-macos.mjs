import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

if (process.platform !== 'darwin') throw new Error('Build the macOS package on macOS.');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const target = 'aarch64-apple-darwin';
function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${path.basename(command)} exited with ${result.status}`);
  return result.stdout;
}
run(process.execPath, [path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'),
  'build', '--target', target, '--bundles', 'app', '--', '--locked']);

const source = path.join(root, 'src-tauri/target', target, 'release/bundle/macos/SuperGPT Connect.app');
const actual = run('/usr/libexec/PlistBuddy', ['-c', 'Print :CFBundleShortVersionString', path.join(source, 'Contents/Info.plist')],
  { encoding: 'utf8', stdio: 'pipe' }).trim();
if (actual !== version) throw new Error('App version does not match package.json');
const binary = fs.readFileSync(path.join(source, 'Contents/MacOS/supergpt-connect'));
if (binary.readUInt32LE(0) !== 0xfeedfacf || binary.readUInt32LE(4) !== 0x0100000c) {
  throw new Error('Expected an Apple Silicon executable');
}
const release = path.join(root, 'release');
fs.mkdirSync(release, { recursive: true });
const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'supergpt-macos-package-'));
try {
  const folder = path.join(stage, 'SuperGPT Connect');
  fs.mkdirSync(folder);
  const app = path.join(folder, 'SuperGPT Connect.app');
  run('/usr/bin/ditto', [source, app]);
  fs.copyFileSync(path.join(root, 'LICENSE'), path.join(app, 'Contents/Resources/LICENSE.txt'));
  fs.copyFileSync(path.join(root, 'LICENSE'), path.join(folder, 'LICENSE.txt'));
  fs.cpSync(path.join(root, 'licenses'), path.join(folder, 'licenses'), { recursive: true });
  run('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', app]);
  run('/usr/bin/codesign', ['--verify', '--deep', '--strict', app]);
  fs.writeFileSync(path.join(folder, 'README.txt'), `SuperGPT Connect ${version} · macOS Apple Silicon

解压后打开 SuperGPT Connect.app，也可拖入「应用程序」文件夹。
选择中转站 → 选择客户端 → 输入 API Key → 获取并勾选模型 → 导入。
导入后完全退出目标客户端，再重新打开并新建会话。
如需撤回，点右上角「恢复原配置」。

当前包使用 ad-hoc 签名，尚未完成 Apple Developer ID 签名及公证。
若系统拦截，请先确认下载自本项目的 GitHub Release，再按系统提示处理。

源码与说明：https://github.com/superpilot69/supergpt-connect
测试范围：https://github.com/superpilot69/supergpt-connect/blob/v${version}/VERIFICATION.md
`);
  const name = `SuperGPT-Connect-${version}-macOS-arm64.zip`;
  const archive = path.join(stage, name);
  run('/usr/bin/ditto', ['-c', '-k', '--norsrc', '--keepParent', folder, archive]);
  fs.copyFileSync(archive, path.join(release, name));
  const hashes = fs.readdirSync(release).filter(name => name.endsWith('.zip')).sort()
    .map(name => `${createHash('sha256').update(fs.readFileSync(path.join(release, name))).digest('hex')}  ${name}`);
  fs.writeFileSync(path.join(release, 'SHA256SUMS.txt'), hashes.join('\n') + '\n');
  console.log(`Packaged: ${path.join(release, name)}`);
} finally {
  fs.rmSync(stage, { recursive: true, force: true });
}
