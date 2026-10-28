import { useMemo, useState } from "react";
import { EditInput } from "./EditInput";
import { Modal } from "./Modal";
import { isDefaultProtocol, protocolSummary } from "../lib/protocol";
import { DEFAULT_PROTOCOL, type ProtocolSettings } from "../lib/types";

const HEAD_SIZE = 32;
const RESPONSE_SIZE = 36;
const MIN_PACKET_SIZE = HEAD_SIZE + 32;
const MAX_PACKET_SIZE = 1_048_576;

interface Draft {
  port: string;
  packetSize: string;
  magic: string;
  packetType: string;
  statusOffset: string;
  connectTimeoutS: string;
  connectAttempts: string;
  recvTimeoutS: string;
}

type DraftKey = keyof Draft;

interface ParsedProtocol {
  protocol: ProtocolSettings | null;
  errors: Partial<Record<DraftKey, string>>;
  preview: {
    port: number | null;
    packetSize: number | null;
    magic: number | null;
    packetType: number | null;
    statusOffset: number | null;
  };
}

function toHex(value: number): string {
  return value.toString(16).toUpperCase().padStart(4, "0");
}

function toHexOffset(value: number): string {
  return `0x${value.toString(16).toUpperCase().padStart(2, "0")}`;
}

function toDraft(protocol: ProtocolSettings): Draft {
  return {
    port: String(protocol.port),
    packetSize: String(protocol.packetSize),
    magic: toHex(protocol.magic),
    packetType: toHex(protocol.packetType),
    statusOffset: String(protocol.statusOffset),
    connectTimeoutS: String(protocol.connectTimeoutS),
    connectAttempts: String(protocol.connectAttempts),
    recvTimeoutS: String(protocol.recvTimeoutS),
  };
}

function parseInteger(value: string, min: number, max: number): number | null {
  const text = value.trim();
  if (!/^\d+$/.test(text)) return null;
  const parsed = Number(text);
  return Number.isSafeInteger(parsed) && parsed >= min && parsed <= max ? parsed : null;
}

function parseHex(value: string, min: number, max: number): number | null {
  const text = value.trim();
  if (!/^[0-9a-fA-F]{1,4}$/.test(text)) return null;
  const parsed = Number.parseInt(text, 16);
  return parsed >= min && parsed <= max ? parsed : null;
}

function parseProtocol(draft: Draft): ParsedProtocol {
  const errors: Partial<Record<DraftKey, string>> = {};
  const port = parseInteger(draft.port, 1, 65535);
  const packetSize = parseInteger(draft.packetSize, MIN_PACKET_SIZE, MAX_PACKET_SIZE);
  const magic = parseHex(draft.magic, 0, 0xffff);
  const packetType = parseHex(draft.packetType, 0, 0xffff);
  const statusOffset = parseInteger(draft.statusOffset, 0, RESPONSE_SIZE - 4);
  const connectTimeoutS = parseInteger(draft.connectTimeoutS, 1, 600);
  const connectAttempts = parseInteger(draft.connectAttempts, 1, 10);
  const recvTimeoutS = parseInteger(draft.recvTimeoutS, 1, 3600);

  if (port === null) errors.port = "1-65535";
  if (packetSize === null) errors.packetSize = `${MIN_PACKET_SIZE}-${MAX_PACKET_SIZE}`;
  if (magic === null) errors.magic = "4 位十六进制";
  if (packetType === null) errors.packetType = "4 位十六进制";
  if (statusOffset === null) errors.statusOffset = `0-${RESPONSE_SIZE - 4}`;
  if (connectTimeoutS === null) errors.connectTimeoutS = "1-600";
  if (connectAttempts === null) errors.connectAttempts = "1-10";
  if (recvTimeoutS === null) errors.recvTimeoutS = "1-3600";

  const protocol =
    Object.keys(errors).length === 0
      ? {
          port: port as number,
          packetSize: packetSize as number,
          magic: magic as number,
          packetType: packetType as number,
          statusOffset: statusOffset as number,
          connectTimeoutS: connectTimeoutS as number,
          connectAttempts: connectAttempts as number,
          recvTimeoutS: recvTimeoutS as number,
        }
      : null;

  return {
    protocol,
    errors,
    preview: { port, packetSize, magic, packetType, statusOffset },
  };
}

interface NumberFieldProps {
  label: string;
  value: string;
  error?: string;
  disabled: boolean;
  mono?: boolean;
  onChange(value: string): void;
}

function NumberField({ label, value, error, disabled, mono, onChange }: NumberFieldProps) {
  return (
    <label className="field proto-field">
      <span className="field-caption">{label}</span>
      <EditInput
        value={value}
        inputMode="numeric"
        spellCheck={false}
        autoComplete="off"
        disabled={disabled}
        aria-label={label}
        aria-invalid={Boolean(error)}
        className={mono ? "mono-input" : undefined}
        onChange={(event) => onChange(event.target.value)}
      />
      {error ? <span className="proto-error">范围 {error}</span> : null}
    </label>
  );
}

interface ProtocolModalProps {
  protocol: ProtocolSettings;
  busy: boolean;
  onSave(protocol: ProtocolSettings): void;
  onDismiss(): void;
}

export function ProtocolModal({ protocol, busy, onSave, onDismiss }: ProtocolModalProps) {
  const [draft, setDraft] = useState<Draft>(() => toDraft(protocol));
  const [configView, setConfigView] = useState<"send" | "receive" | null>(null);
  const parsed = useMemo(() => parseProtocol(draft), [draft]);
  const { preview, errors } = parsed;

  const update = (key: DraftKey, value: string) => {
    setDraft((previous) => ({ ...previous, [key]: value }));
  };

  const port = preview.port ?? 0;
  const packetSize = preview.packetSize ?? 0;
  const payloadSize = Math.max(0, packetSize - HEAD_SIZE);
  const magic = preview.magic === null ? "—" : `0x${toHex(preview.magic)}`;
  const packetType = preview.packetType === null ? "—" : `0x${toHex(preview.packetType)}`;
  const statusOffset = preview.statusOffset ?? 32;
  const responsePrefix = Math.max(0, Math.min(statusOffset, RESPONSE_SIZE - 4));
  const responseSuffix = Math.max(0, RESPONSE_SIZE - responsePrefix - 4);

  const headerRows = [
    { offset: 0, size: 2, type: "u16 LE", name: "魔数", value: magic, configurable: true },
    { offset: 2, size: 2, type: "u16 LE", name: "保留", value: "0x0000", configurable: false },
    { offset: 4, size: 4, type: "u32 LE", name: "文件总长", value: "发送时自动填入", configurable: false },
    { offset: 8, size: 4, type: "u32 LE", name: "本包长度", value: "发送时自动填入", configurable: false },
    { offset: 12, size: 4, type: "u32 LE", name: "包序号", value: "从 0 开始递增", configurable: false },
    { offset: 16, size: 2, type: "u16 LE", name: "类型", value: packetType, configurable: true },
    { offset: 18, size: 2, type: "u16 LE", name: "保留", value: "0x0000", configurable: false },
    { offset: 20, size: 12, type: "字节", name: "保留", value: "全部 0x00", configurable: false },
  ];

  const sendTable = (
    <table className="proto-table">
      <thead>
        <tr>
          <th>偏移</th>
          <th>长度</th>
          <th>类型</th>
          <th>字段</th>
          <th>当前值 / 说明</th>
        </tr>
      </thead>
      <tbody>
        {headerRows.map((row) => (
          <tr key={row.offset} className={row.configurable ? "configurable" : undefined}>
            <td>
              <code>{toHexOffset(row.offset)}</code>
            </td>
            <td>{row.size} B</td>
            <td>{row.type}</td>
            <td>{row.name}</td>
            <td>
              <b>{row.value}</b>
              {row.configurable ? <span className="proto-tag">可配置</span> : null}
            </td>
          </tr>
        ))}
        <tr>
          <td>
            <code>0x20</code>
          </td>
          <td>{packetSize > 0 ? `${payloadSize} B` : "—"}</td>
          <td>原始数据</td>
          <td>PAK 数据</td>
          <td>
            <b>按分包大小切分文件</b>
          </td>
        </tr>
      </tbody>
    </table>
  );

  const receiveTable = (
    <table className="proto-table">
      <thead>
        <tr>
          <th>偏移</th>
          <th>长度</th>
          <th>类型</th>
          <th>字段</th>
          <th>当前值 / 说明</th>
        </tr>
      </thead>
      <tbody>
        {responsePrefix > 0 ? (
          <tr>
            <td>
              <code>0x00</code>
            </td>
            <td>{responsePrefix} B</td>
            <td>—</td>
            <td>回显 / 保留</td>
            <td>
              <b>内容不校验</b>
            </td>
          </tr>
        ) : null}
        <tr className="configurable">
          <td>
            <code>{preview.statusOffset === null ? "—" : toHexOffset(statusOffset)}</code>
          </td>
          <td>4 B</td>
          <td>u32 LE</td>
          <td>状态码</td>
          <td>
            <b>0 = 成功，非 0 = 失败码</b>
            <span className="proto-tag">偏移可配置</span>
          </td>
        </tr>
        {responseSuffix > 0 ? (
          <tr>
            <td>
              <code>{toHexOffset(statusOffset + 4)}</code>
            </td>
            <td>{responseSuffix} B</td>
            <td>—</td>
            <td>尾部</td>
            <td>
              <b>内容不校验</b>
            </td>
          </tr>
        ) : null}
      </tbody>
    </table>
  );

  const handleSave = () => {
    if (parsed.protocol) onSave(parsed.protocol);
  };

  return (
    <>
      <Modal
        title="AQ300 升级协议"
        className="protocol-modal"
        onDismiss={onDismiss}
        buttons={[
          {
            label: "恢复默认",
            disabled: busy,
            onClick: () => setDraft(toDraft(DEFAULT_PROTOCOL)),
          },
          { label: "取消", onClick: onDismiss },
          {
            label: "保存",
            primary: true,
            disabled: busy || !parsed.protocol,
            onClick: handleSave,
          },
        ]}
      >
        <div className="proto-body">
          <div className="proto-intro">
            <span className="eyebrow">TCP :{port || "—"} · 小端序</span>
            <p className="proto-current">
              当前生效：{protocolSummary(protocol)}
              {isDefaultProtocol(protocol) ? "" : "（已自定义）"}
            </p>
            <p>传输参数在本页修改；发送 / 接收报文字段在对应窗口中配置。</p>
          </div>

          <section className="proto-section" aria-label="传输参数">
            <h3>传输参数</h3>
            <div className="proto-grid">
              <NumberField
                label={`TCP 端口 · 默认 ${DEFAULT_PROTOCOL.port}`}
                value={draft.port}
                error={errors.port}
                disabled={busy}
                onChange={(value) => update("port", value)}
              />
              <NumberField
                label={`分包大小 (B) · 默认 ${DEFAULT_PROTOCOL.packetSize}`}
                value={draft.packetSize}
                error={errors.packetSize}
                disabled={busy}
                onChange={(value) => update("packetSize", value)}
              />
              <NumberField
                label={`连接超时 (s) · 默认 ${DEFAULT_PROTOCOL.connectTimeoutS}`}
                value={draft.connectTimeoutS}
                error={errors.connectTimeoutS}
                disabled={busy}
                onChange={(value) => update("connectTimeoutS", value)}
              />
              <NumberField
                label={`连接重试 · 默认 ${DEFAULT_PROTOCOL.connectAttempts}`}
                value={draft.connectAttempts}
                error={errors.connectAttempts}
                disabled={busy}
                onChange={(value) => update("connectAttempts", value)}
              />
              <NumberField
                label={`响应超时 (s) · 默认 ${DEFAULT_PROTOCOL.recvTimeoutS}`}
                value={draft.recvTimeoutS}
                error={errors.recvTimeoutS}
                disabled={busy}
                onChange={(value) => update("recvTimeoutS", value)}
              />
            </div>
          </section>

          <div className="proto-launch-grid">
            <button
              type="button"
              className="proto-launch"
              disabled={busy}
              onClick={() => setConfigView("send")}
            >
              <b>配置发送协议</b>
              <span>32 B 头 + 数据 · 魔数 / 类型</span>
            </button>
            <button
              type="button"
              className="proto-launch"
              disabled={busy}
              onClick={() => setConfigView("receive")}
            >
              <b>配置接收协议</b>
              <span>36 B 响应 · 状态码偏移</span>
            </button>
          </div>
        </div>
      </Modal>

      {configView === "send" ? (
        <Modal
          title="配置发送协议"
          className="protocol-modal protocol-wide"
          onDismiss={() => setConfigView(null)}
          buttons={[{ label: "完成", primary: true, onClick: () => setConfigView(null) }]}
        >
          <div className="proto-body">
            <div className="proto-intro">
              <span className="eyebrow">发送 · 小端序</span>
              <p className="proto-current">
                当前魔数 {magic} · 类型 {packetType}
              </p>
              <p>修改后下方表格实时预览，保存并关闭主窗口后生效。</p>
            </div>

            <section className="proto-section" aria-label="发送参数">
              <h3>发送参数</h3>
              <div className="proto-grid">
                <NumberField
                  label={`魔数 (hex) · 默认 0x${toHex(DEFAULT_PROTOCOL.magic)}`}
                  value={draft.magic}
                  error={errors.magic}
                  disabled={busy}
                  mono
                  onChange={(value) => update("magic", value)}
                />
                <NumberField
                  label={`类型 (hex) · 默认 0x${toHex(DEFAULT_PROTOCOL.packetType)}`}
                  value={draft.packetType}
                  error={errors.packetType}
                  disabled={busy}
                  mono
                  onChange={(value) => update("packetType", value)}
                />
              </div>
            </section>

            <section className="proto-section" aria-label="发送格式">
              <h3>发送格式 · 每包 = 32 B 头 + 数据</h3>
              <div className="proto-frame">
                <div className="proto-frame-bar">
                  <div className="proto-seg proto-seg-head" style={{ flex: "0 0 88px" }}>
                    <span>头</span>
                    <b>32 B</b>
                  </div>
                  <div className="proto-seg proto-seg-data" style={{ flex: "1 1 auto" }}>
                    <span>数据</span>
                    <b>{packetSize > 0 ? `${payloadSize} B` : "—"}</b>
                  </div>
                </div>
                <p className="proto-caption">
                  {packetSize > 0
                    ? `每包 ${packetSize} B，其中数据 ${payloadSize} B（结构示意，不按比例）。`
                    : "请先在主窗口输入有效的分包大小。"}
                </p>
              </div>
              {sendTable}
            </section>
          </div>
        </Modal>
      ) : null}

      {configView === "receive" ? (
        <Modal
          title="配置接收协议"
          className="protocol-modal protocol-wide"
          onDismiss={() => setConfigView(null)}
          buttons={[{ label: "完成", primary: true, onClick: () => setConfigView(null) }]}
        >
          <div className="proto-body">
            <div className="proto-intro">
              <span className="eyebrow">接收 · 小端序</span>
              <p className="proto-current">当前状态码偏移 {statusOffset}</p>
              <p>设备固定返回 36 字节，程序按下方偏移读取状态码。</p>
            </div>

            <section className="proto-section" aria-label="接收参数">
              <h3>接收参数</h3>
              <div className="proto-grid">
                <NumberField
                  label={`状态码偏移 · 默认 ${DEFAULT_PROTOCOL.statusOffset}`}
                  value={draft.statusOffset}
                  error={errors.statusOffset}
                  disabled={busy}
                  onChange={(value) => update("statusOffset", value)}
                />
              </div>
            </section>

            <section className="proto-section" aria-label="接收格式">
              <h3>接收格式 · 固定读取 36 B</h3>
              {receiveTable}
              <p className="proto-caption">
                发送完全部数据后等待设备返回 36 字节，按上表偏移读取状态码。
              </p>
            </section>
          </div>
        </Modal>
      ) : null}
    </>
  );
}
