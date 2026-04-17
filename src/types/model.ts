export interface ModelInfo {
  description: string;
  n_params: number;
  n_ctx_train: number;
}

export interface ModelRef {
  repo_id: string;
  filename: string;
}

export interface ModelEntry {
  repo_id: string;
  filename: string;
  size_bytes: number;
  downloaded_at: string;
}

export interface DownloadProgress {
  repo_id: string;
  filename: string;
  bytes_downloaded: number;
  total_bytes: number | null;
}

export interface GgufFileInfo {
  filename: string;
  size_bytes: number | null;
  quantization_label: string | null;
}

export interface HubSearchResult {
  repo_id: string;
  author?: string | null;
  downloads: number;
  likes: number;
  gguf_files: GgufFileInfo[];
}

export interface ModelReady {
  model_info: ModelInfo;
  repo_id: string;
  filename: string;
}

export interface StartupState {
  status: "loading" | "ready" | "failed";
  model_info?: ModelInfo;
  loading_model?: ModelRef;
  loaded_model?: ModelRef;
  error?: string;
}
