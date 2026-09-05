export type SandboxStatus = "pending_runtime" | "running" | "stopped" | "destroyed" | string;

export interface Sandbox {
  id: string;
  tenant_id: string;
  status: SandboxStatus;
  created_at: string;
  destroyed_at: string | null;
}

export interface CreateSandboxRequest {
  vcpu_count?: number;
  memory_mib?: number;
  exec_timeout_secs?: number;
}

export interface ApiError {
  status: number;
  message: string;
}
