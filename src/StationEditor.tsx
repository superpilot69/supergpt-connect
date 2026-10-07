import { useEffect, useRef, useState, type FormEvent } from "react";
import { ArrowRight, CircleAlert, Globe2, Trash2, X } from "lucide-react";
import { normalizeStationUrl, type Station } from "./stations";

export default function StationEditor({
  station,
  stations,
  onSave,
  onDelete,
  onClose,
}: {
  station: Station | null;
  stations: Station[];
  onSave: (station: Station) => void;
  onDelete: (id: string) => void;
  onClose: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [name, setName] = useState(station?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(station?.baseUrl ?? "");
  const [error, setError] = useState("");
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    element.querySelector<HTMLInputElement>("#station-name")?.focus();
    return () => element.close();
  }, []);
  function submit(event: FormEvent) {
    event.preventDefault();
    try {
      const trimmed = name.trim();
      if (
        !trimmed ||
        trimmed.length > 32 ||
        /[\u0000-\u001f\u007f]/.test(trimmed)
      ) {
        throw new Error("请填写 1–32 个字符的站点名称。");
      }
      const normalized = normalizeStationUrl(baseUrl);
      if (
        stations.some(
          (item) => item.id !== station?.id && item.baseUrl === normalized,
        )
      ) {
        throw new Error("这个 API 地址已在列表中，请直接选择已有站点。");
      }
      if (
        stations.some(
          (item) =>
            item.id !== station?.id &&
            item.name.toLowerCase() === trimmed.toLowerCase(),
        )
      ) {
        throw new Error("这个名称已被使用，换一个更容易区分的名称吧。");
      }
      if (!station && stations.length >= 31)
        throw new Error("最多保存 30 个自定义站点，请先删除不再使用的站点。");
      onSave({
        id: station?.id ?? `custom-${crypto.randomUUID()}`,
        name: trimmed,
        baseUrl: normalized,
      });
    } catch (error) {
      setError(
        error instanceof Error
          ? error.message
          : "站点信息无法保存，请检查后重试。",
      );
    }
  }
  return (
    <dialog
      ref={dialog}
      className="station-dialog"
      aria-labelledby="station-dialog-title"
      onCancel={onClose}
    >
      <div className="dialog-heading">
        <span className="dialog-icon">
          <Globe2 size={22} />
        </span>
        <button
          className="icon-button"
          type="button"
          onClick={onClose}
          aria-label="关闭站点设置"
        >
          <X size={18} />
        </button>
      </div>
      <h2 id="station-dialog-title">{station ? "编辑中转站" : "添加中转站"}</h2>
      <p className="dialog-intro">保存名称与 API 地址，方便下次选择。</p>
      <form onSubmit={submit} noValidate>
        <label htmlFor="station-name">站点名称</label>
        <input
          id="station-name"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setError("");
          }}
          placeholder="例如：我的中转站"
          maxLength={32}
          autoComplete="off"
          autoFocus
        />
        <label htmlFor="station-url">API 地址</label>
        <input
          id="station-url"
          type="url"
          value={baseUrl}
          onChange={(e) => {
            setBaseUrl(e.target.value);
            setError("");
          }}
          placeholder="https://api.example.com"
          maxLength={2048}
          spellCheck={false}
          autoCapitalize="none"
          autoComplete="off"
        />
        <p className="field-hint">
          填写服务商提供的 API 基础地址，可包含 /v1。
        </p>
        {error && (
          <div className="notice notice-error dialog-error" role="alert">
            <CircleAlert size={16} />
            <p>{error}</p>
          </div>
        )}
        {station && (
          <p className="delete-hint">
            删除仅移除列表记录，已导入的客户端配置不受影响。
          </p>
        )}
        <div className="dialog-actions">
          {station && (
            <button
              type="button"
              className="delete-button"
              onClick={() => onDelete(station.id)}
            >
              <Trash2 size={15} />
              删除
            </button>
          )}
          <button type="button" className="secondary-button" onClick={onClose}>
            取消
          </button>
          <button type="submit" className="primary-button">
            {station ? "保存修改" : "保存并选择"}
            <ArrowRight size={16} />
          </button>
        </div>
      </form>
    </dialog>
  );
}
