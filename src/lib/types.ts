export type ThemeName = "dark" | "light";

export interface AppConfig {
  ip: string;
  filePath: string;
  ftpHost: string;
  ftpUsername: string;
  ftpPassword: string;
  ftpFolder: string;
  useLocal: boolean;
  theme: ThemeName;
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
