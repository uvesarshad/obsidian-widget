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
  theme: 'system' | 'light' | 'dark';
  opacity: number;
  shortcut: string;
  reminder_tone: string;
  reminder_tone_path: string;
}

export interface Reminder {
  id: string;
  task_text: string;
  remind_at: string; // "YYYY-MM-DDTHH:MM"
  fired: boolean;
}

export interface NoteItem {
  line_idx: number;
  kind: 'task' | 'heading' | 'separator' | 'text' | 'bullet';
  text: string;
  done: boolean;
  task_id: number;
  level: number;
  indent: number;
}
