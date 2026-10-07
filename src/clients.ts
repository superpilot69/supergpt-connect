import type { Client } from "./native";

export const families = [
  { id: "codex", name: "Codex", maker: "OpenAI", hint: "桌面应用 · 终端" },
  { id: "claude", name: "Claude", maker: "Anthropic", hint: "桌面应用 · Claude Code" },
] as const;

export const clients: { id: Client; name: string; label: string; hint: string }[] = [
  { id: "codex-desktop", name: "Codex 桌面版", label: "桌面版", hint: "本地会话，与终端共用配置" },
  { id: "codex", name: "Codex 终端版", label: "终端版", hint: "CLI，与桌面版共用配置" },
  { id: "claude-desktop", name: "Claude 桌面版", label: "桌面版", hint: "Chat · Cowork · 本地 Code" },
  { id: "claude", name: "Claude Code 终端版", label: "终端版", hint: "Claude Code CLI，也适用于 IDE 内终端" },
  { id: "claude-vscode", name: "Claude Code · VS Code", label: "VS Code 扩展", hint: "VS Code 默认用户配置" },
];

export function clientFamily(client: Client): "codex" | "claude" {
  return client.startsWith("codex") ? "codex" : "claude";
}

export function modelSelectable(client: Client, id: string): boolean {
  return client !== "claude-desktop" || /^(anthropic\/)?claude-(sonnet|opus|haiku|fable)-[^\[]+$/i.test(id);
}
