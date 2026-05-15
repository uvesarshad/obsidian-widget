export interface Task {
  id: number;
  done: boolean;
  text: string;
  line_idx: number;
}

export interface WindowConfig {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Settings {
  vault_path: string;
  target_file: string;
  window: WindowConfig;
  always_on_top: boolean;
  click_through_on_blur: boolean;
}
