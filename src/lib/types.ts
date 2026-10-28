export type ThemeName = "dark" | "light";

export interface ProtocolSettings {
  port: number;
  packetSize: number;
  magic: number;
  packetType: number;
  statusOffset: number;
  connectTimeoutS: number;
  connectAttempts: number;
  recvTimeoutS: number;
}

export const DEFAULT_PROTOCOL: ProtocolSettings = {
  port: 9000,
  packetSize: 4096,
  magic: 0x9000,
  packetType: 2,
  statusOffset: 32,
  connectTimeoutS: 12,
  connectAttempts: 3,
  recvTimeoutS: 120,
};

export interface AppConfig {
  ip: string;
  filePath: string;
  ftpHost: string;
  ftpUsername: string;
  ftpPassword: string;
  ftpFolder: string;
  useLocal: boolean;
  theme: ThemeName;
  protocol: ProtocolSettings;
}

export const DEFAULT_CONFIG: AppConfig = {
  ip: "172.16.10.110",
  filePath: "Z:/",
  ftpHost: "172.16.9.6",
  ftpUsername: "lyp",
  ftpPassword: "123456",
  ftpFolder: "/",
  useLocal: true,
  theme: "dark",
  protocol: DEFAULT_PROTOCOL,
};

export interface WindowGeometry {
  width: number;
  height: number;
  x: number;
  y: number;
  maximized: boolean;
}

export interface SessionState {
  config: AppConfig;
  window: WindowGeometry | null;
}

export interface AppPaths {
  appDir: string;
  dataDir: string;
  tmpDir: string;
  sessionFile: string;
}

export interface ProgressUpdate {
  bytes: number;
  total: number;
  percent: number;
}

export interface CaptionColours {
  background: string;
  foreground: string;
}

export interface SystemCaption {
  accent: string | null;
  caption: CaptionColours | null;
}

export const NO_SYSTEM_CAPTION: SystemCaption = { accent: null, caption: null };

export interface ChromeMetrics {
  titleHeight: number;
  menuHeight: number;
  menuFontSize: number;
  tabHeight: number;
  measured: boolean;
}
