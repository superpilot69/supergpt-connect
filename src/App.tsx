import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  ChevronRight,
  CircleAlert,
  Eye,
  EyeOff,
  Globe2,
  KeyRound,
  LoaderCircle,
  LockKeyhole,
  Moon,
  Pencil,
  Plus,
  RotateCcw,
  Search,
  ShieldCheck,
  Sun,
  Terminal,
  X,
} from "lucide-react";
import {
  discoverModels,
  importConfig,
  nativeAvailable,
  readStatus,
  type Client,
  type DiscoveryResult,
  type ImportResult,
  type Status,
} from "./native";
import {
  DEFAULT_STATION,
  loadStationPreferences,
  saveStationPreferences,
  stationAddress,
  type Station,
  type StationPreferences,
} from "./stations";
import {
  EMPTY_SELECTION,
  MAX_MODELS,
  selectModels,
  type ModelSelection,
} from "./selection";
import StationEditor from "./StationEditor";
import RestorePanel from "./RestorePanel";
import { version } from "../package.json";

const steps = ["中转站", "客户端", "API 密钥", "选择模型"];
import { clients, families, clientFamily, modelSelectable } from "./clients";
function BrandIcon({ className = "" }: { className?: string }) {
  return (
    <img
      className={`brand-icon ${className}`}
      src="/supergpt.png"
      alt=""
      draggable={false}
    />
  );
}
function ClientIcon({ client, size = 24 }: { client: Client; size?: number }) {
  return clientFamily(client) === "codex" ? (
    <Terminal size={size} strokeWidth={1.6} />
  ) : (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
    >
      {[0, 30, 60, 90, 120, 150].map((angle) => (
        <path
          key={angle}
          d="M12 2.5V21.5"
          stroke="currentColor"
          strokeWidth="1.8"
          transform={`rotate(${angle} 12 12)`}
        />
      ))}
    </svg>
  );
}
function Modal({
  title,
  children,
  close,
}: {
  title: string;
  children: ReactNode;
  close: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const node = ref.current!;
    node.showModal();
    return () => node.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="station-dialog"
      aria-label={title}
      onCancel={close}
    >
      <div className="modal-title">
        <h2>{title}</h2>
        <button className="icon-button" onClick={close} aria-label="关闭对话框">
          <X size={18} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
function initialTheme(): "light" | "dark" {
  try {
    return localStorage.getItem("supergpt-connect.theme") === "dark"
      ? "dark"
      : "light";
  } catch {
    return "light";
  }
}
export default function App() {
  const [theme, setTheme] = useState(initialTheme);
  const [step, setStep] = useState(0);
  const [furthest, setFurthest] = useState(0);
  const [preferences, setPreferences] = useState<StationPreferences>(() => ({
    ...loadStationPreferences(),
    selected: DEFAULT_STATION.id,
  }));
  const [editing, setEditing] = useState<Station | null | undefined>();
  const [client, setClient] = useState<Client>("codex-desktop");
  const [apiKey, setApiKey] = useState("");
  const [visible, setVisible] = useState(false);
  const [discovery, setDiscovery] = useState<DiscoveryResult | null>(null);
  const [selection, setSelection] = useState<ModelSelection>(EMPTY_SELECTION);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState<"discover" | "import" | "restore" | null>(
    null,
  );
  const [error, setError] = useState("");
  const [result, setResult] = useState<ImportResult | null>(null);
  const [restored, setRestored] = useState(false);
  const [showRestore, setShowRestore] = useState(false);
  const [about, setAbout] = useState(false);
  const heading = useRef<HTMLHeadingElement>(null);
  const stations = [DEFAULT_STATION, ...preferences.custom];
  const station =
    stations.find((item) => item.id === preferences.selected) ??
    DEFAULT_STATION;
  const selectedClient = clients.find((item) => item.id === client)!;
  const clientStatus = status?.clients.find((item) => item.client === client);
  const disabled = busy !== null;
  const family = clientFamily(client);
  const matching =
    discovery?.models.filter((model) =>
      `${model.id} ${model.name}`
        .toLowerCase()
        .includes(query.trim().toLowerCase()),
    ) ?? [];
  const selectableMatching = matching.filter((model) =>
    modelSelectable(client, model.id),
  );
  const allMatchingSelected =
    selectableMatching.length > 0 &&
    selectableMatching.every((model) => selection.ids.includes(model.id));

  async function refresh() {
    if (nativeAvailable) setStatus(await readStatus());
  }
  useEffect(() => {
    refresh().catch(() => setError("无法读取客户端状态，请重新打开连接器。"));
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("supergpt-connect.theme", theme);
    } catch {
      /* Session theme still works. */
    }
  }, [theme]);
  useEffect(() => {
    window.scrollTo({ top: 0 });
    heading.current?.focus({ preventScroll: true });
  }, [step, result]);
  function clearDiscovery() {
    setDiscovery(null);
    setSelection(EMPTY_SELECTION);
    setQuery("");
    setError("");
    setResult(null);
    setRestored(false);
  }
  function changePreferences(next: StationPreferences) {
    clearDiscovery();
    setApiKey("");
    setVisible(false);
    setFurthest(0);
    setPreferences(next);
    if (!saveStationPreferences(next))
      setError("站点已在本次会话中选中，但无法保存到本机。");
  }
  function saveStation(next: Station) {
    changePreferences({
      version: 1,
      selected: next.id,
      custom: preferences.custom.some((item) => item.id === next.id)
        ? preferences.custom.map((item) => (item.id === next.id ? next : item))
        : [...preferences.custom, next],
    });
    setEditing(undefined);
  }
  function changeClient(next: Client) {
    if (next === client) return;
    setClient(next);
    clearDiscovery();
    setApiKey("");
    setVisible(false);
    setFurthest(1);
  }
  function go(next: number) {
    setStep(next);
    setFurthest((current) => Math.max(current, next));
    setError("");
  }
  async function fetchModels(event?: FormEvent) {
    event?.preventDefault();
    if (disabled || !apiKey.trim()) return;
    clearDiscovery();
    if (!nativeAvailable) {
      setError("这是界面预览，请在桌面应用中获取模型并导入。");
      return;
    }
    setBusy("discover");
    try {
      const fetched = await discoverModels({
        client,
        baseUrl: station.baseUrl,
        apiKey: apiKey.trim(),
      });
      setDiscovery(fetched);
      go(3);
    } catch (error) {
      setError(typeof error === "string" ? error : "获取失败，请稍后重试。");
    } finally {
      setBusy(null);
    }
  }
  async function importSelected() {
    if (disabled || !discovery || !selection.ids.length) return;
    setBusy("import");
    setError("");
    try {
      const imported = await importConfig({
        discoveryId: discovery.discoveryId,
        selectedModels: selection.ids,
        defaultModel: selection.defaultId,
      });
      setResult(imported);
      setApiKey("");
      setVisible(false);
      setDiscovery(null);
      try {
        await refresh();
      } catch {
        setError("配置已导入，但状态刷新失败。请重新打开应用查看备份。");
      }
    } catch (error) {
      setError(typeof error === "string" ? error : "导入失败，请稍后重试。");
    } finally {
      setBusy(null);
    }
  }
  async function restoredClient(next: Client) {
    clearDiscovery();
    setClient(next === "codex" ? "codex-desktop" : next);
    setApiKey("");
    setVisible(false);
    setRestored(true);
    setStep(0);
    setFurthest(0);
    await refresh();
  }
  function startAgain() {
    clearDiscovery();
    setApiKey("");
    setVisible(false);
    setStep(0);
    setFurthest(0);
    setPreferences((current) => ({ ...current, selected: DEFAULT_STATION.id }));
  }
  function toggleModel(id: string, checked: boolean) {
    setSelection((current) =>
      selectModels(
        current,
        checked
          ? [...current.ids, id]
          : current.ids.filter((value) => value !== id),
      ),
    );
  }
  return (
    <main className={`app-shell ${step === 1 ? "client-step" : ""}`}>
      <header className="app-header">
        <div className="brand">
          <BrandIcon />
          <span className="brand-wordmark">supergpt</span>
          <span className="brand-divider" />
          <span className="brand-product">Connect</span>
        </div>
        <div className="header-actions">
          <button
            className="restore-header-button"
            onClick={() => setShowRestore(true)}
            disabled={disabled}
          >
            <RotateCcw size={15} />
            <span>恢复原配置</span>
          </button>
          <button
            className="icon-button theme-button"
            onClick={() => setTheme(theme === "light" ? "dark" : "light")}
            aria-label={theme === "light" ? "切换深色外观" : "切换浅色外观"}
            title={theme === "light" ? "深色外观" : "浅色外观"}
          >
            {theme === "light" ? <Moon size={17} /> : <Sun size={17} />}
          </button>
        </div>
      </header>
      <div className="workspace">
        <div className="page-intro">
          <div>
            <h1>连接你的 AI 工具</h1>
            <p>从密钥到模型，按你的选择配置。</p>
          </div>
          <span className="local-label">
            <span />
            本地配置
          </span>
        </div>
        {(!nativeAvailable || status?.sandbox) && (
          <div className="preview-label">
            {!nativeAvailable
              ? "界面预览 · 获取模型和导入需使用桌面版"
              : "测试模式 · 使用独立配置目录"}
          </div>
        )}
        <nav className="stepper" aria-label="配置步骤">
          <ol>
            {steps.map((title, index) => (
              <li
                key={title}
                className={`${step === index && !result ? "current" : ""} ${index < step || result ? "complete" : ""}`}
              >
                <button
                  disabled={
                    disabled ||
                    !!result ||
                    index > furthest ||
                    (index === 3 && !discovery)
                  }
                  onClick={() => go(index)}
                  aria-current={step === index && !result ? "step" : undefined}
                >
                  <span className="step-circle">
                    {index < step || result ? (
                      <Check size={13} strokeWidth={2.5} />
                    ) : (
                      index + 1
                    )}
                  </span>
                  <span>{title}</span>
                </button>
              </li>
            ))}
          </ol>
        </nav>
        <section
          className={`wizard-card ${step === 3 ? "models-card" : ""}`}
          aria-labelledby="step-title"
          aria-busy={disabled}
        >
          {result ? (
            <div className="success-content" role="status">
              <span className="success-mark">
                <Check size={28} strokeWidth={1.7} />
              </span>
              <h2 id="step-title" ref={heading} tabIndex={-1}>
                配置已写入，等待客户端重启
              </h2>
              <p>{selectedClient.name} · 已保存中转站和所选模型</p>
              <div className="result-summary">
                <div>
                  <span>中转站</span>
                  <strong>{station.name}</strong>
                </div>
                <div>
                  <span>默认模型</span>
                  <strong className="mono">{result.model}</strong>
                </div>
                <div>
                  <span>已导入模型</span>
                  <strong>{result.selectedModels.length} 个</strong>
                </div>
              </div>
              <div className="result-models">
                {result.selectedModels.map((id) => (
                  <span key={id}>{id}</span>
                ))}
              </div>
              <div className="activation-guide">
                <strong>接下来</strong>
                <ol>
                  {result.nextSteps.map((text) => (
                    <li key={text}>{text}</li>
                  ))}
                </ol>
                <p>已验证配置写入；尚未验证客户端实际发出请求。</p>
              </div>
              {result.warnings.length > 0 && (
                <details className="compatibility-details">
                  <summary>兼容性提示</summary>
                  {result.warnings.map((text) => (
                    <p key={text}>{text}</p>
                  ))}
                </details>
              )}
            </div>
          ) : (
            <>
              <div className="card-heading">
                <span className="eyebrow">
                  STEP 0{step + 1} <span>/ 04</span>
                </span>
                <h2 id="step-title" ref={heading} tabIndex={-1}>
                  {
                    [
                      "选择中转站",
                      "选择要导入的客户端",
                      "输入你的 API 密钥",
                      "选择要导入的模型",
                    ][step]
                  }
                </h2>
                <p>
                  {
                    [
                      "默认使用 SuperGPT，也可以选择其他中转站。",
                      "先选择产品，再选择你实际使用的入口。",
                      `使用 ${station.name} 的密钥，获取你可以选择的模型。`,
                      "勾选需要的模型，并指定一个默认模型。",
                    ][step]
                  }
                </p>
              </div>
              {step === 0 && (
                <div className="step-content station-content">
                  <fieldset className="station-list" disabled={disabled}>
                    <legend className="sr-only">选择中转站（单选）</legend>
                    {stations.map((item) => (
                      <div
                        key={item.id}
                        className={`station-row ${station.id === item.id ? "is-selected" : ""}`}
                      >
                        <label className="station-option">
                          <input
                            type="radio"
                            name="station"
                            value={item.id}
                            checked={station.id === item.id}
                            onChange={() => {
                              if (item.id !== station.id)
                                changePreferences({
                                  ...preferences,
                                  selected: item.id,
                                });
                            }}
                          />
                          <span className="station-emblem">
                            {item.id === DEFAULT_STATION.id ? (
                              <BrandIcon />
                            ) : (
                              <Globe2 size={24} strokeWidth={1.5} />
                            )}
                          </span>
                          <span className="station-copy">
                            <span className="station-name">
                              {item.name}
                              {item.id === DEFAULT_STATION.id && (
                                <span className="badge">默认</span>
                              )}
                            </span>
                            <span className="station-address">
                              {stationAddress(item)}
                            </span>
                          </span>
                          <span className="radio-indicator" aria-hidden="true">
                            <span />
                          </span>
                        </label>
                        {item.id !== DEFAULT_STATION.id && (
                          <button
                            className="icon-button edit-station"
                            aria-label={`编辑 ${item.name}`}
                            onClick={() => setEditing(item)}
                            disabled={disabled}
                          >
                            <Pencil size={15} />
                          </button>
                        )}
                      </div>
                    ))}
                  </fieldset>
                  <button
                    className="add-station"
                    onClick={() => setEditing(null)}
                    disabled={disabled}
                  >
                    <span className="add-icon">
                      <Plus size={20} strokeWidth={1.5} />
                    </span>
                    <span>
                      <strong>添加其他中转站</strong>
                      <small>填写名称和 API 地址</small>
                    </span>
                    <ChevronRight size={17} />
                  </button>
                </div>
              )}
              {step === 1 && (
                <div className="step-content">
                  <fieldset className="client-grid" disabled={disabled}>
                    <legend className="sr-only">选择客户端</legend>
                    {families.map((item) => (
                      <label
                        key={item.id}
                        className={`client-option ${family === item.id ? "is-selected" : ""}`}
                      >
                        <input
                          type="radio"
                          name="client"
                          checked={family === item.id}
                          onChange={() =>
                            changeClient(
                              item.id === "codex"
                                ? "codex-desktop"
                                : "claude-desktop",
                            )
                          }
                          value={item.id}
                        />
                        <span className="client-top">
                          <span className="client-icon">
                            <ClientIcon client={item.id} size={27} />
                          </span>
                          <span className="radio-indicator" aria-hidden="true">
                            <span />
                          </span>
                        </span>
                        <span className="client-name">{item.name}</span>
                        <span className="client-maker">{item.maker}</span>
                        <span className="client-hint">{item.hint}</span>
                      </label>
                    ))}
                  </fieldset>
                  <fieldset className="edition-list" disabled={disabled}>
                    <legend>你使用哪个入口？</legend>
                    {clients
                      .filter((item) => clientFamily(item.id) === family)
                      .map((item) => (
                        <label
                          key={item.id}
                          className={`edition-option ${client === item.id ? "is-selected" : ""}`}
                        >
                          <input
                            type="radio"
                            name="edition"
                            checked={client === item.id}
                            onChange={() => changeClient(item.id)}
                          />
                          <span>
                            <strong>{item.label}</strong>
                            <small>{item.hint}</small>
                          </span>
                          <span className="edition-check">
                            {client === item.id && <Check size={15} />}
                          </span>
                        </label>
                      ))}
                  </fieldset>
                  <div className="client-detection">
                    <span
                      className={`detection-dot ${clientStatus?.compatibility.installed ? "found" : ""}`}
                    />
                    <span>
                      {!nativeAvailable
                        ? "桌面应用会自动检测安装情况"
                        : !clientStatus
                          ? "正在检查客户端…"
                          : clientStatus.compatibility.installed
                            ? `已检测到 ${selectedClient.name}${clientStatus.compatibility.version ? ` ${clientStatus.compatibility.version}` : ""}`
                            : "未在常用位置检测到，可先保存配置"}
                    </span>
                  </div>
                  {clientStatus?.compatibility.blocked && (
                    <p className="inline-note blocking-note">
                      {clientStatus.compatibility.blocked}
                    </p>
                  )}
                  {!!clientStatus?.compatibility.warnings.length && (
                    <details className="compatibility-details">
                      <summary>查看兼容性说明</summary>
                      {clientStatus.compatibility.warnings.map((text) => (
                        <p key={text}>{text}</p>
                      ))}
                    </details>
                  )}
                  <p className="inline-note">
                    仅配置本机入口；Claude
                    网页版、手机端和云端任务不读取本机配置。
                  </p>
                </div>
              )}
              {step === 2 && (
                <form
                  id="key-form"
                  className="step-content key-content"
                  onSubmit={fetchModels}
                >
                  <div className="connection-route">
                    <span className="route-station">
                      {station.id === DEFAULT_STATION.id ? (
                        <BrandIcon />
                      ) : (
                        <Globe2 size={22} />
                      )}
                      <strong>{station.name}</strong>
                    </span>
                    <ArrowRight size={17} />
                    <span>
                      <ClientIcon client={client} size={20} />
                      <strong>{selectedClient.name}</strong>
                    </span>
                  </div>
                  <label className="field-label" htmlFor="api-key">
                    API Key
                  </label>
                  <div className="secret-field">
                    <KeyRound size={18} />
                    <input
                      id="api-key"
                      name="api-key"
                      type={visible ? "text" : "password"}
                      value={apiKey}
                      onChange={(event) => {
                        setApiKey(event.target.value);
                        clearDiscovery();
                        setFurthest(2);
                      }}
                      placeholder="粘贴你的 API 密钥"
                      autoComplete="off"
                      autoCapitalize="none"
                      spellCheck={false}
                      maxLength={4096}
                      disabled={disabled}
                      aria-describedby="key-destination"
                    />
                    <button
                      type="button"
                      className="icon-button"
                      aria-label={visible ? "隐藏密钥" : "显示密钥"}
                      aria-pressed={visible}
                      onClick={() => setVisible(!visible)}
                      disabled={disabled}
                    >
                      {visible ? <EyeOff size={17} /> : <Eye size={17} />}
                    </button>
                  </div>
                  <p id="key-destination" className="key-destination">
                    <LockKeyhole size={13} />
                    <span>
                      密钥仅发送至 <b>{stationAddress(station)}</b>
                    </span>
                  </p>
                  <div className="key-note">
                    <span className="note-number">下一步</span>
                    <p>
                      读取当前密钥的模型列表，
                      <br />
                      由你决定导入哪些模型。
                    </p>
                  </div>
                </form>
              )}
              {step === 3 && discovery && (
                <div className="step-content model-content">
                  <div className="model-toolbar">
                    <div className="search-field">
                      <Search size={16} />
                      <input
                        aria-label="搜索模型"
                        placeholder="搜索模型名称…"
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        disabled={disabled}
                        autoComplete="off"
                      />
                    </div>
                    <span className="model-count">
                      {discovery.models.length} 个模型
                    </span>
                  </div>
                  <div className="list-heading">
                    <span>
                      {query ? `找到 ${matching.length} 个` : "可用模型"}
                    </span>
                    <button
                      className="text-button"
                      disabled={disabled || !selectableMatching.length}
                      onClick={() =>
                        setSelection((current) =>
                          selectModels(
                            current,
                            allMatchingSelected
                              ? current.ids.filter(
                                  (id) =>
                                    !selectableMatching.some(
                                      (model) => model.id === id,
                                    ),
                                )
                              : [
                                  ...current.ids,
                                  ...selectableMatching.map(
                                    (model) => model.id,
                                  ),
                                ],
                          ),
                        )
                      }
                    >
                      {allMatchingSelected
                        ? "取消选择"
                        : query
                          ? "选择搜索结果"
                          : "全选"}
                    </button>
                  </div>
                  <div
                    className="model-list"
                    role="group"
                    aria-label="勾选要导入的模型"
                  >
                    {matching.map((model) => {
                      const checked = selection.ids.includes(model.id);
                      const isDefault = model.id === selection.defaultId;
                      return (
                        <div
                          key={model.id}
                          className={`model-row ${checked ? "is-checked" : ""}`}
                        >
                          <label>
                            <input
                              type="checkbox"
                              checked={checked}
                              disabled={
                                !modelSelectable(client, model.id) ||
                                disabled ||
                                (!checked && selection.ids.length >= MAX_MODELS)
                              }
                              onChange={(event) =>
                                toggleModel(model.id, event.target.checked)
                              }
                              aria-label={`导入 ${model.id}`}
                            />
                            <span className="checkbox-mark" aria-hidden="true">
                              {checked && <Check size={12} strokeWidth={2.7} />}
                            </span>
                            <span className="model-name">
                              <strong>{model.name}</strong>
                              {model.name !== model.id && (
                                <small>{model.id}</small>
                              )}
                            </span>
                          </label>
                          {!modelSelectable(client, model.id) && (
                            <span className="model-unavailable">
                              不兼容桌面版
                            </span>
                          )}
                          {checked && (
                            <button
                              className={`default-model ${isDefault ? "active" : ""}`}
                              aria-label={`将 ${model.id} 设为默认模型`}
                              aria-pressed={isDefault}
                              onClick={() =>
                                setSelection((current) => ({
                                  ...current,
                                  defaultId: model.id,
                                }))
                              }
                              disabled={disabled}
                            >
                              {isDefault ? (
                                <>
                                  <span />
                                  默认模型
                                </>
                              ) : (
                                "设为默认"
                              )}
                            </button>
                          )}
                        </div>
                      );
                    })}
                    {!matching.length && (
                      <div className="empty-models">
                        <Search size={22} />
                        <p>没有找到匹配的模型</p>
                        <button
                          className="text-button"
                          onClick={() => setQuery("")}
                        >
                          清空搜索
                        </button>
                      </div>
                    )}
                  </div>
                  <div className="selection-summary" aria-live="polite">
                    <span>
                      已选择 <strong>{selection.ids.length}</strong> 个模型
                      {selection.ids.length >= MAX_MODELS && "（已达上限）"}
                    </span>
                    {selection.ids.length > 0 ? (
                      <span className="default-summary">
                        默认：
                        <b title={selection.defaultId}>{selection.defaultId}</b>
                      </span>
                    ) : (
                      <span>请至少勾选一个</span>
                    )}
                  </div>
                  <p className="fine-print">
                    模型列表来自当前密钥，实际调用需中转站支持{" "}
                    {family === "codex" ? "Responses" : "Messages"} 接口。
                  </p>
                </div>
              )}
            </>
          )}
          {(error || clientStatus?.pending || restored) && (
            <div className="feedback-area">
              {clientStatus?.pending && (
                <div className="notice notice-error" role="alert">
                  <CircleAlert size={16} />
                  <p>上次导入被中断，请先通过下方入口恢复配置。</p>
                </div>
              )}
              {error && (
                <div className="notice notice-error" role="alert">
                  <CircleAlert size={16} />
                  <p>{error}</p>
                  <button
                    className="icon-button"
                    aria-label="关闭提示"
                    onClick={() => setError("")}
                  >
                    <X size={15} />
                  </button>
                </div>
              )}
              {restored && (
                <div className="notice notice-success" role="status">
                  <Check size={17} />
                  <p>已恢复 {selectedClient.name} 导入前的配置。</p>
                </div>
              )}
            </div>
          )}
          <div className="card-actions">
            {result ? (
              <>
                <span className="action-hint">
                  <ShieldCheck size={14} />
                  原配置已备份
                </span>
                <button className="primary-button" onClick={startAgain}>
                  配置另一个客户端
                  <ArrowRight size={16} />
                </button>
              </>
            ) : (
              <>
                {step > 0 ? (
                  <button
                    className="back-button"
                    onClick={() => go(step - 1)}
                    disabled={disabled}
                  >
                    <ArrowLeft size={15} />
                    上一步
                  </button>
                ) : (
                  <span className="action-hint">每次连接只使用一个中转站</span>
                )}
                {step < 2 && (
                  <button
                    className="primary-button"
                    disabled={
                      disabled ||
                      (step === 1 && !!clientStatus?.compatibility.blocked)
                    }
                    onClick={() => go(step + 1)}
                  >
                    下一步
                    <ArrowRight size={16} />
                  </button>
                )}
                {step === 2 && (
                  <button
                    type="submit"
                    form="key-form"
                    className="primary-button"
                    disabled={disabled || !apiKey.trim()}
                  >
                    {busy === "discover" ? (
                      <>
                        <LoaderCircle className="spinning" size={16} />
                        正在获取模型…
                      </>
                    ) : (
                      <>
                        获取可用模型
                        <ArrowRight size={16} />
                      </>
                    )}
                  </button>
                )}
                {step === 3 && (
                  <button
                    className="primary-button"
                    disabled={
                      disabled ||
                      !selection.ids.length ||
                      !discovery ||
                      !!clientStatus?.pending ||
                      !!clientStatus?.compatibility.blocked
                    }
                    onClick={importSelected}
                  >
                    {busy === "import" ? (
                      <>
                        <LoaderCircle className="spinning" size={16} />
                        正在导入…
                      </>
                    ) : (
                      <>
                        导入 {selectedClient.name}
                        <ArrowRight size={16} />
                      </>
                    )}
                  </button>
                )}
              </>
            )}
          </div>
        </section>
        <div className="below-card">
          <span>
            <ShieldCheck size={14} />
            导入前自动备份原配置
          </span>
          {clientStatus?.hasBackup && (
            <button
              className="text-button"
              onClick={() => setShowRestore(true)}
              disabled={disabled}
            >
              <RotateCcw size={13} />
              恢复 {selectedClient.name} 配置
            </button>
          )}
        </div>
      </div>
      <footer className="app-footer">
        <span>SuperGPT Connect</span>
        <button onClick={() => setAbout(true)} className="version-button">
          v{version} <span>·</span> 关于
        </button>
      </footer>
      {editing !== undefined && (
        <StationEditor
          station={editing}
          stations={stations}
          onSave={saveStation}
          onDelete={(id) => {
            changePreferences({
              version: 1,
              selected: DEFAULT_STATION.id,
              custom: preferences.custom.filter((item) => item.id !== id),
            });
            setEditing(undefined);
          }}
          onClose={() => setEditing(undefined)}
        />
      )}
      {showRestore && (
        <RestorePanel
          initialClient={client}
          onClose={() => setShowRestore(false)}
          onRestored={restoredClient}
        />
      )}
      {about && (
        <Modal title="SuperGPT Connect" close={() => setAbout(false)}>
          <p className="dialog-intro">v{version} · 本地 API 配置器</p>
          <p className="about-copy">
            基于 CC Switch 的配置引擎与模型适配逻辑，采用 MIT 许可。原作者 Jason
            Young，完整许可随应用附带。
          </p>
          <p className="about-copy">
            仅保存站点名称和地址。密钥仅用于所选站点的请求及客户端配置，不写入浏览器存储。
          </p>
          <div className="dialog-actions">
            <button className="primary-button" onClick={() => setAbout(false)}>
              知道了
            </button>
          </div>
        </Modal>
      )}
    </main>
  );
}
