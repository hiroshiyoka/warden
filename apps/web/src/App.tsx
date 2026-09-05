import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createSandbox, listSandboxes } from "./lib/api";
import type { ApiError, CreateSandboxRequest, Sandbox } from "./lib/types";

export default function App() {
  const [view, setView] = useState<"list" | "create">("list");
  return (
    <div className="page">
      <header>
        <h1>Warden sandbox dashboard</h1>
        <nav>
          <button type="button" onClick={() => setView("list")} aria-current={view === "list"}>
            Sandboxes
          </button>
          <button type="button" onClick={() => setView("create")} aria-current={view === "create"}>
            New sandbox
          </button>
        </nav>
      </header>
      <main>{view === "list" ? <SandboxList /> : <SandboxCreate onCreated={() => setView("list")} />}</main>
    </div>
  );
}

function SandboxList() {
  const query = useQuery<Sandbox[], ApiError>({
    queryKey: ["sandboxes"],
    queryFn: listSandboxes,
    refetchOnWindowFocus: false,
  });

  if (query.isLoading) return <p>Loading…</p>;
  if (query.isError) return <ErrorPanel error={query.error} />;

  const sandboxes = query.data ?? [];
  return (
    <section>
      <h2>Sandboxes</h2>
      {sandboxes.length === 0 ? (
        <p>No sandboxes yet.</p>
      ) : (
        <ul className="list">
          {sandboxes.map((sandbox) => (
            <li key={sandbox.id}>
              <code>{sandbox.id}</code>
              <span className={`status status-${sandbox.status}`}>{describeStatus(sandbox.status)}</span>
              <time dateTime={sandbox.created_at}>{new Date(sandbox.created_at).toLocaleString()}</time>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function describeStatus(status: string): string {
  if (status === "pending_runtime") {
    return "Pending runtime — no execution backend yet";
  }
  return status;
}

function SandboxCreate({ onCreated }: { onCreated: () => void }) {
  const queryClient = useQueryClient();
  const mutation = useMutation<Sandbox, ApiError, CreateSandboxRequest>({
    mutationFn: createSandbox,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sandboxes"] });
      onCreated();
    },
  });

  return (
    <section>
      <h2>New sandbox</h2>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const form = new FormData(event.currentTarget);
          mutation.mutate({
            vcpu_count: Number(form.get("vcpu_count")) || 1,
            memory_mib: Number(form.get("memory_mib")) || 512,
            exec_timeout_secs: Number(form.get("exec_timeout_secs")) || 30,
          });
        }}
      >
        <label>
          vCPUs
          <input name="vcpu_count" type="number" min={1} max={4} defaultValue={1} />
        </label>
        <label>
          Memory (MiB)
          <input name="memory_mib" type="number" min={64} max={2048} defaultValue={512} />
        </label>
        <label>
          Exec timeout (seconds)
          <input name="exec_timeout_secs" type="number" min={1} max={300} defaultValue={30} />
        </label>
        <button type="submit" disabled={mutation.isPending}>
          {mutation.isPending ? "Submitting…" : "Request sandbox"}
        </button>
      </form>
      {mutation.isError ? <RuntimeNotice error={mutation.error} /> : null}
    </section>
  );
}

function RuntimeNotice({ error }: { error: ApiError }) {
  if (error.status === 503) {
    return (
      <p className="notice">
        Sandbox request recorded. Execution runtime isn&apos;t deployed yet — the API returned 503.
      </p>
    );
  }
  return <ErrorPanel error={error} />;
}

function ErrorPanel({ error }: { error: ApiError }) {
  if (error.status === 401) {
    return (
      <p className="notice">
        The API requires an authenticated session, but the dashboard has no login UI yet. Phase 2A ships
        without one; talk to the API directly or wait for a follow-up.
      </p>
    );
  }
  if (error.status === 429) {
    return <p className="notice">Sandbox quota exceeded for this tenant. The API returned 429.</p>;
  }
  if (error.status === 403) {
    return <p className="notice">Permission denied. The API returned 403.</p>;
  }
  return (
    <p className="notice">
      API request failed ({error.status}): {error.message}
    </p>
  );
}
