export interface Station {
  id: string;
  name: string;
  baseUrl: string;
}
export interface StationPreferences {
  version: 1;
  selected: string;
  custom: Station[];
}
export const DEFAULT_STATION: Station = {
  id: "supergpt",
  name: "SuperGPT",
  baseUrl: "https://api.supergpt.dev",
};
export const STATION_STORAGE_KEY = "supergpt-connect.stations.v1";

export function normalizeStationUrl(value: string): string {
  if (value.trim().length > 2048)
    throw new Error("API 地址过长，请检查后重试。");
  let url: URL;
  try {
    url = new URL(value.trim());
  } catch {
    throw new Error("请填写完整的 API 地址，例如 https://api.example.com。");
  }
  const local = ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
  if (url.protocol !== "https:" && !(url.protocol === "http:" && local)) {
    throw new Error("请使用 HTTPS 地址；本机调试可使用 HTTP。");
  }
  if (!url.hostname || url.username || url.password || url.search || url.hash) {
    throw new Error("地址中不能包含账号、密钥、查询参数或锚点。");
  }
  const path = url.pathname.replace(/\/+$/, "");
  if (/\/(responses|messages|chat\/completions)$/.test(path)) {
    throw new Error(
      "请填写 API 基础地址，不要包含 /responses 或 /messages 等接口路径。",
    );
  }
  url.pathname = path.replace(/\/v1$/, "");
  return url.toString().replace(/\/+$/, "");
}
export function stationAddress(station: Station): string {
  return station.baseUrl.replace(/^https?:\/\//, "");
}
function emptyPreferences(): StationPreferences {
  return { version: 1, selected: DEFAULT_STATION.id, custom: [] };
}
// Persist names and public API addresses only, never keys or imported config.
export function parseStationPreferences(
  raw: string | null,
): StationPreferences {
  const fallback = emptyPreferences();
  if (!raw) return fallback;
  try {
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object") return fallback;
    const data = value as Record<string, unknown>;
    if (data.version !== 1 || !Array.isArray(data.custom)) return fallback;
    const seenIds = new Set([DEFAULT_STATION.id]);
    const seenUrls = new Set([DEFAULT_STATION.baseUrl]);
    for (const row of data.custom.slice(0, 30)) {
      if (!row || typeof row !== "object") continue;
      const { id, name, baseUrl } = row;
      if (
        typeof id !== "string" ||
        !/^custom-[a-zA-Z0-9-]{1,80}$/.test(id) ||
        seenIds.has(id) ||
        typeof name !== "string" ||
        !name.trim() ||
        name.trim().length > 32 ||
        /[\u0000-\u001f\u007f]/.test(name) ||
        typeof baseUrl !== "string"
      )
        continue;
      try {
        const normalized = normalizeStationUrl(baseUrl);
        if (seenUrls.has(normalized)) continue;
        fallback.custom.push({ id, name: name.trim(), baseUrl: normalized });
        seenIds.add(id);
        seenUrls.add(normalized);
      } catch {
        /* Ignore malformed entries without blocking the app. */
      }
    }
    if (typeof data.selected === "string" && seenIds.has(data.selected))
      fallback.selected = data.selected;
    return fallback;
  } catch {
    return fallback;
  }
}
export function loadStationPreferences(): StationPreferences {
  try {
    return parseStationPreferences(localStorage.getItem(STATION_STORAGE_KEY));
  } catch {
    return emptyPreferences();
  }
}
export function saveStationPreferences(
  preferences: StationPreferences,
): boolean {
  try {
    const safe = parseStationPreferences(JSON.stringify(preferences));
    localStorage.setItem(STATION_STORAGE_KEY, JSON.stringify(safe));
    return true;
  } catch {
    return false;
  }
}
