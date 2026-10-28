import { DEFAULT_PROTOCOL, type ProtocolSettings } from "./types";

function hex(value: number): string {
  return `0x${value.toString(16).toUpperCase().padStart(4, "0")}`;
}

export function isDefaultProtocol(protocol: ProtocolSettings): boolean {
  return (Object.keys(DEFAULT_PROTOCOL) as Array<keyof ProtocolSettings>).every(
    (key) => protocol[key] === DEFAULT_PROTOCOL[key],
  );
}

export function protocolSummary(protocol: ProtocolSettings): string {
  return [
    `端口 ${protocol.port}`,
    `分包 ${protocol.packetSize} B`,
    `魔数 ${hex(protocol.magic)}`,
    `类型 ${hex(protocol.packetType)}`,
    `状态码偏移 ${protocol.statusOffset}`,
  ].join(" · ");
}
