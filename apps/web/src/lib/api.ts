import type { ApiError, CreateSandboxRequest, Sandbox } from "./types";

const API_BASE = import.meta.env.VITE_API_URL ?? "";

export async function listSandboxes(): Promise<Sandbox[]> {
  const response = await fetch(`${API_BASE}/sandboxes`, { credentials: "include" });
  return unwrap(response);
}

export async function createSandbox(payload: CreateSandboxRequest): Promise<Sandbox> {
  const response = await fetch(`${API_BASE}/sandboxes`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "include",
    body: JSON.stringify(payload),
  });
  return unwrap(response);
}

async function unwrap(response: Response): Promise<never> {
  const message = await safeErrorMessage(response);
  const error: ApiError = { status: response.status, message };
  throw error;
}

async function safeErrorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string };
    if (body.error) return body.error;
  } catch {
    // fall through
  }
  return response.statusText || `HTTP ${response.status}`;
}
