/**
 * Settings — API keys (Phase 2.2).
 *
 * Keys are stored in the OS keychain by the backend. This panel can set,
 * clear, and see whether a key exists — it never receives a stored key back.
 */

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { KeyRound, Check, Trash2, Save } from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type { SecretProvider } from "@/lib/bindings";

const PROVIDERS: Array<{ id: SecretProvider; label: string; note: string }> = [
  {
    id: "anthropic",
    label: "Anthropic (Claude)",
    note: "Brukes av AI-tegnsetting, smarte forslag og oversettelse.",
  },
  { id: "open-ai", label: "OpenAI", note: "Sky-transkripsjon (kommer)." },
  {
    id: "assembly-ai",
    label: "AssemblyAI",
    note: "Sky-transkripsjon (kommer).",
  },
  { id: "deepgram", label: "Deepgram", note: "Sky-transkripsjon (kommer)." },
];

export function SettingsPanel() {
  const statusQuery = useQuery({
    queryKey: ["secret-status"],
    queryFn: () => ipc.secrets.status(),
  });
  const status = statusQuery.data ?? [];
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [msg, setMsg] = useState<string | null>(null);

  const present = (id: SecretProvider) =>
    status.find((s) => s.provider === id)?.present ?? false;

  async function save(id: SecretProvider) {
    const value = (drafts[id] ?? "").trim();
    if (!value) return;
    setMsg(null);
    try {
      await ipc.secrets.set(id, value);
      setDrafts((d) => ({ ...d, [id]: "" }));
      await statusQuery.refetch();
      setMsg("Nøkkel lagret i nøkkelringen.");
    } catch (e) {
      setMsg(e instanceof IPCError ? `Feil: ${e.message}` : String(e));
    }
  }

  async function clear(id: SecretProvider) {
    setMsg(null);
    try {
      await ipc.secrets.delete(id);
      await statusQuery.refetch();
      setMsg("Nøkkel fjernet.");
    } catch (e) {
      setMsg(e instanceof IPCError ? `Feil: ${e.message}` : String(e));
    }
  }

  return (
    <div className="mx-auto max-w-2xl p-6">
      <div className="mb-1 flex items-center gap-2">
        <KeyRound size={18} className="text-[var(--color-accent-400)]" />
        <h2 className="text-[var(--text-ui-lg)] font-semibold">API-nøkler</h2>
      </div>
      <p className="mb-6 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        Nøkler lagres i operativsystemets nøkkelring (Keychain på Mac,
        Credential Manager på Windows) — aldri i klartekst og aldri i
        prosjektfiler. SundayEdit viser kun om en nøkkel er satt, ikke selve
        verdien.
      </p>

      <ul className="space-y-3">
        {PROVIDERS.map((p) => {
          const isSet = present(p.id);
          return (
            <li
              key={p.id}
              className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-3"
            >
              <div className="mb-2 flex items-center gap-2">
                <span className="text-[var(--text-ui-sm)] font-medium">
                  {p.label}
                </span>
                {isSet && (
                  <span className="flex items-center gap-1 rounded-full bg-[var(--color-success)]/15 px-2 py-0.5 text-[10px] text-[var(--color-success)]">
                    <Check size={10} /> satt
                  </span>
                )}
              </div>
              <p className="mb-2 text-[10px] text-[var(--color-fg-subtle)]">
                {p.note}
              </p>
              <div className="flex items-center gap-2">
                <input
                  type="password"
                  value={drafts[p.id] ?? ""}
                  placeholder={isSet ? "•••••••• (lagret)" : "Lim inn nøkkel"}
                  onChange={(e) =>
                    setDrafts((d) => ({ ...d, [p.id]: e.target.value }))
                  }
                  className="flex-1 rounded-md border border-[var(--color-border)] bg-[var(--color-bg-surface)] px-2.5 py-1.5 font-mono text-[var(--text-ui-sm)] outline-none focus:border-[var(--color-accent-500)]"
                />
                <button
                  type="button"
                  onClick={() => save(p.id)}
                  disabled={!(drafts[p.id] ?? "").trim()}
                  className="flex items-center gap-1.5 rounded-md bg-[var(--color-accent-600)] px-3 py-1.5 text-[var(--text-ui-xs)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-40"
                >
                  <Save size={12} /> Lagre
                </button>
                {isSet && (
                  <button
                    type="button"
                    onClick={() => clear(p.id)}
                    title="Fjern nøkkel"
                    aria-label="Fjern nøkkel"
                    className="rounded-md p-1.5 text-[var(--color-fg-subtle)] hover:bg-[var(--color-danger)]/10 hover:text-[var(--color-danger)]"
                  >
                    <Trash2 size={14} />
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>

      {msg && (
        <p className="mt-4 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
          {msg}
        </p>
      )}
    </div>
  );
}
