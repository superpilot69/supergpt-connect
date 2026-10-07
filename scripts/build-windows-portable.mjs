// Build and package without an installer, development tools, or user configuration files.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const packageOnly = process.argv.includes('--package-only');
if (process.argv.slice(2).some(arg => arg !== '--package-only')) {
  throw new Error('Usage: node scripts/build-windows-portable.mjs [--package-only]');
}
const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const target = 'x86_64-pc-windows-msvc';
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit', env: process.env });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${path.basename(command)} exited with ${result.status}`);
}
if (!packageOnly) {
  const args = [path.join(root, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', '--target', target, '--no-bundle'];
  if (process.platform !== 'win32') args.push('--runner', 'cargo-xwin');
  args.push('--', '--locked');
  run(process.execPath, args);
}

const executable = path.join(root, 'src-tauri/target', target, 'release/supergpt-connect.exe');
const pe = fs.readFileSync(executable);
if (pe.length < 512 || pe.toString('ascii', 0, 2) !== 'MZ') throw new Error('Missing Windows PE executable');
if (!pe.includes(Buffer.from(version + '\0', 'utf16le'))) throw new Error('EXE version does not match package.json; rebuild first');
if (!pe.includes(Buffer.from('asInvoker'))) throw new Error('EXE must declare that administrator elevation is not required');
const header = pe.readUInt32LE(0x3c);
if (pe.readUInt32LE(header) !== 0x4550 || pe.readUInt16LE(header + 4) !== 0x8664) {
  throw new Error('The portable package requires a Windows x64 executable');
}
const optional = header + 24;
if (pe.readUInt16LE(optional) !== 0x20b || pe.readUInt16LE(optional + 68) !== 2) {
  throw new Error('Expected PE32+ with the Windows GUI subsystem');
}
const sections = header + 24 + pe.readUInt16LE(header + 20);
function rvaOffset(rva) {
  for (let i = 0; i < pe.readUInt16LE(header + 6); i++) {
    const section = sections + i * 40;
    const start = pe.readUInt32LE(section + 12);
    const size = Math.max(pe.readUInt32LE(section + 8), pe.readUInt32LE(section + 16));
    if (rva >= start && rva < start + size) return pe.readUInt32LE(section + 20) + rva - start;
  }
  throw new Error(`Invalid PE RVA ${rva}`);
}
const imports = [];
for (let offset = rvaOffset(pe.readUInt32LE(optional + 120)); pe.readUInt32LE(offset + 12); offset += 20) {
  const start = rvaOffset(pe.readUInt32LE(offset + 12));
  imports.push(pe.toString('ascii', start, pe.indexOf(0, start)));
}
const extraRuntime = imports.filter(name => /^(vcruntime|msvcp|concrt|ucrtbase|api-ms-win-crt|webview2loader)/i.test(name));
if (extraRuntime.length) throw new Error(`Portable EXE still requires unbundled runtime DLLs: ${extraRuntime.join(', ')}`);

const release = path.join(root, 'release');
fs.mkdirSync(release, { recursive: true });
const stage = fs.mkdtempSync(path.join(os.tmpdir(), 'supergpt-windows-package-'));
const folder = path.join(stage, 'SuperGPT Connect');
const archiveName = `SuperGPT-Connect-${version}-Windows-x64-portable.zip`;
try {
  fs.mkdirSync(folder);
  fs.copyFileSync(executable, path.join(folder, 'SuperGPT Connect.exe'));
  fs.copyFileSync(path.join(root, 'LICENSE'), path.join(folder, 'LICENSE.txt'));
  fs.cpSync(path.join(root, 'licenses'), path.join(folder, 'licenses'), { recursive: true });
  const instructions = `SuperGPT Connect ${version} · Windows 免安装版

适用：Windows 10 / 11，64 位 Intel / AMD 电脑。

使用方法
1. 先把整个压缩包解压到一个文件夹。
2. 双击「SuperGPT Connect.exe」。无需管理员权限，无需安装 Node.js 或 Rust。
3. 选择中转站 → 选择客户端 → 输入自己的 API Key → 获取并勾选模型 → 导入。
4. 完全退出 Claude / Codex 后重新打开，再新建会话。
   Claude 如留在系统托盘中，请从托盘菜单退出。
5. 如需撤回，点右上角「恢复原配置」，选择客户端后点「恢复」。

如果无法打开
本应用使用 Microsoft Edge WebView2 Runtime。若提示缺少它，请从微软官网安装
Evergreen Standalone Installer 的 x64 版本，然后重新打开应用：
https://developer.microsoft.com/microsoft-edge/webview2/
安装 WebView2 只在系统缺少此组件时需要；连接器本身不需要安装。
当前版本尚未进行 Windows 发布者代码签名，系统可能提示未知发布者。

确认配置生效
检查窗口、模型列表和导入是否正常；重启目标客户端后发送一条测试消息，
再用「恢复原配置」检查能否回到导入前。仅看到“导入成功”不代表客户端已切换。

说明
Claude / Codex 客户端需要另外安装。本工具只配置当前 Windows 用户的本机客户端；
WSL、远程开发环境有自己的配置，须在对应环境另行设置。
应用关闭后不会常驻。配置与恢复记录保存在当前用户目录；删除 EXE 不会自动恢复配置。
${process.platform === 'win32' ? '本包在 Windows 上构建。' : '本包由其他系统交叉编译。'}测试范围见：
https://github.com/superpilot69/supergpt-connect/blob/v${version}/VERIFICATION.md

源码与许可：https://github.com/superpilot69/supergpt-connect
`;
  // ASCII filenames also extract correctly in older Windows ZIP tools; contents are Chinese UTF-8.
  fs.writeFileSync(path.join(folder, 'README.txt'), '\ufeff' + instructions.replace(/\n/g, '\r\n'));
  const stagedArchive = path.join(stage, archiveName);
  if (process.platform === 'darwin') {
    run('/usr/bin/ditto', ['-c', '-k', '--norsrc', '--keepParent', folder, stagedArchive]);
  } else if (process.platform === 'win32') {
    run('tar.exe', ['-a', '-cf', stagedArchive, 'SuperGPT Connect'], stage);
  } else {
    run('zip', ['-q', '-r', stagedArchive, 'SuperGPT Connect'], stage);
  }
  fs.copyFileSync(stagedArchive, path.join(release, archiveName));
  const hashes = fs.readdirSync(release).filter(name => name.endsWith('.zip')).sort()
    .map(name => `${createHash('sha256').update(fs.readFileSync(path.join(release, name))).digest('hex')}  ${name}`);
  fs.writeFileSync(path.join(release, 'SHA256SUMS.txt'), hashes.join('\n') + '\n');
  console.log(`Windows x64 GUI verified; no external VC++ or WebView2Loader DLL required.`);
  console.log(`Packaged: ${path.join(release, archiveName)}`);
  console.log(`Size: ${(fs.statSync(path.join(release, archiveName)).size / 1024 / 1024).toFixed(2)} MiB`);
} finally {
  fs.rmSync(stage, { recursive: true, force: true });
}
