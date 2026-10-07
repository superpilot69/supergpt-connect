import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, CircleAlert, LoaderCircle, X } from "lucide-react";
import {
  nativeAvailable,
  readRestorePoints,
  restoreConfig,
  type Client,
  type RestorePoint,
  type RestoreResult,
} from "./native";

const targets: { client: Client; name: string }[] = [
  { client: "claude-desktop", name: "Claude 桌面版" },
  { client: "claude", name: "Claude Code · 终端" },
  { client: "claude-vscode", name: "Claude Code · VS Code" },
  { client: "codex", name: "Codex · 桌面与终端" },
];

export default function RestorePanel({
  initialClient,
  onClose,
  onRestored,
}: {
  initialClient: Client;
  onClose: () => void;
  onRestored: (client: Client) => Promise<void>;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [points, setPoints] = useState<RestorePoint[]>([]);
  const [selected, setSelected] = useState<Client>(
    initialClient === "codex-desktop" ? "codex" : initialClient,
  );
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [result, setResult] = useState<Pick<
    RestoreResult,
    "client" | "wasPending"
  > | null>(null);
  const point = points.find((p) => p.client === selected);
  const choices = targets.filter((target) =>
    points.some((p) => p.client === target.client && (p.available || p.error)),
  );
  const target = targets.find((t) => t.client === selected)!;
  const changed =
    point?.files.some((file) => file.modifiedSinceImport) ?? false;

  async function refresh() {
    setLoading(true);
    setError("");
    setOverwrite(false);
    try {
      const found = nativeAvailable ? await readRestorePoints() : [];
      setPoints(found);
      setSelected((current) => {
        if (found.some((p) => p.client === current && (p.available || p.error)))
          return current;
        return (
          found.find((p) => p.available)?.client ??
          found.find((p) => p.error)?.client ??
          current
        );
      });
    } catch (error) {
      setError(
        typeof error === "string" ? error : "暂时无法检查配置，请重试。",
      );
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    void refresh();
    return () => element?.close();
  }, []);

  async function restore() {
    if (
      busy ||
      loading ||
      !point?.available ||
      !point.reviewId ||
      (changed && !overwrite)
    )
      return;
    setBusy(true);
    setError("");
    try {
      const done = await restoreConfig(selected, point.reviewId, overwrite);
      setResult({ client: done.client, wasPending: done.wasPending });
      try {
        await onRestored(done.client);
      } catch {
        setError("配置已恢复，请重新打开连接器刷新状态。");
      }
    } catch (error) {
      setError(
        typeof error === "string" ? error : "恢复未完成，请重新检查后重试。",
      );
    } finally {
      setBusy(false);
    }
  }

  const problem = error || point?.error;
  return (
    <dialog
      ref={dialog}
      className="station-dialog restore-dialog"
      aria-labelledby="restore-title"
      onCancel={(event) => {
        if (busy) event.preventDefault();
        else onClose();
      }}
    >
      <div className="restore-heading">
        <h2 id="restore-title">恢复原配置</h2>
        <button
          className="icon-button"
          aria-label="关闭恢复窗口"
          onClick={onClose}
          disabled={busy}
        >
          <X size={18} />
        </button>
      </div>
      <div className="restore-body">
        {result ? (
          <div className="restore-complete" role="status">
            <span className="restore-done-icon">
              <Check size={22} />
            </span>
            <h3>{result.wasPending ? "已撤回未完成的导入" : "已恢复"}</h3>
            <p>
              {result.wasPending
                ? "还可以继续恢复到最初导入前。"
                : `重新打开 ${target.name} 即可生效。`}
            </p>
          </div>
        ) : loading ? (
          <p className="restore-status" role="status">
            <LoaderCircle size={16} className="spinning" />
            正在检查配置…
          </p>
        ) : choices.length > 0 ? (
          <>
            <p className="restore-description">
              撤回连接器的修改，回到导入前。
            </p>
            <label className="restore-client-label" htmlFor="restore-client">
              选择客户端
            </label>
            <div className="restore-client-control">
              <select
                id="restore-client"
                className="restore-client-select"
                value={selected}
                onChange={(event) => {
                  setSelected(event.target.value as Client);
                  setOverwrite(false);
                  setError("");
                }}
                disabled={busy}
              >
                {choices.map((item) => (
                  <option key={item.client} value={item.client}>
                    {item.name}
                  </option>
                ))}
              </select>
              <ChevronDown size={16} aria-hidden="true" />
            </div>
            {point?.available && (
              <>
                {point.pending && (
                  <p className="restore-hint">
                    上次导入未完成，将先撤回那次修改。
                  </p>
                )}
                {changed && (
                  <label className="restore-conflict">
                    <input
                      type="checkbox"
                      checked={overwrite}
                      onChange={(event) => setOverwrite(event.target.checked)}
                      disabled={busy}
                    />
                    <span>同时撤销导入后的配置改动</span>
                  </label>
                )}
                <p className="restore-hint">
                  请先退出对应客户端，恢复后重新打开。
                </p>
              </>
            )}
          </>
        ) : !problem ? (
          <p className="restore-status">
            {nativeAvailable
              ? "暂无可恢复的配置。"
              : "请在桌面应用中使用恢复功能。"}
          </p>
        ) : null}
        {problem && (
          <div className="notice notice-error" role="alert">
            <CircleAlert size={16} />
            <div>
              <p>{problem}</p>
              {!result && (
                <button
                  className="text-button restore-retry"
                  onClick={() => void refresh()}
                  disabled={busy || loading}
                >
                  重新检查
                </button>
              )}
            </div>
          </div>
        )}
      </div>
      <div className="restore-actions">
        {result ? (
          <>
            {result.wasPending && (
              <button
                className="text-button"
                onClick={() => {
                  setResult(null);
                  void refresh();
                }}
                disabled={busy}
              >
                继续恢复
              </button>
            )}
            <button
              className="primary-button"
              onClick={onClose}
              disabled={busy}
            >
              完成
            </button>
          </>
        ) : !loading && choices.length === 0 && !problem ? (
          <button className="primary-button" onClick={onClose}>
            知道了
          </button>
        ) : (
          <>
            <button
              className="secondary-button"
              onClick={onClose}
              disabled={busy}
            >
              取消
            </button>
            <button
              className="primary-button"
              onClick={restore}
              disabled={
                busy ||
                loading ||
                !point?.available ||
                !!problem ||
                (changed && !overwrite)
              }
            >
              {busy ? (
                <>
                  <LoaderCircle size={16} className="spinning" />
                  正在恢复…
                </>
              ) : (
                "恢复"
              )}
            </button>
          </>
        )}
      </div>
    </dialog>
  );
}
