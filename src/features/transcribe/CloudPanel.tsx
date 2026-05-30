/**
 * Cloud transcription picker — Phase 2.2.
 *
 * Local Whisper is the default and the privacy story; cloud is strictly
 * opt-in. This panel shows the providers with per-minute price, an estimated
 * cost for THIS project's duration, whether an API key is set (from the
 * keychain), and a consent dialog that must be accepted before a provider is
 * selected ("your audio will be uploaded to X").
 *
 * All three providers (OpenAI Whisper, AssemblyAI, Deepgram) transcribe live
 * once a key is set; the returned captions are lifted into the editor.
 */

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Cloud,
  Check,
  ShieldAlert,
  ExternalLink,
  KeyRound,
  Loader2,
} from "lucide-react";

import { ipc, IPCError } from "@/lib/ipc";
import type {
  Caption,
  CloudProvider,
  CloudProviderInfo,
  Project,
  SecretProvider,
} from "@/lib/bindings";
import { useT } from "@/lib/i18n";
import { cn } from "@/lib/cn";

interface Props {
  project: Project;
  onTranscribed: (captions: Caption[]) => void;
}

// CloudProvider → the keychain SecretProvider that holds its key.
const KEY_FOR: Record<CloudProvider, SecretProvider> = {
  "openai-whisper": "open-ai",
  "assembly-ai": "assembly-ai",
  deepgram: "deepgram",
};

async function openPrivacy(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    // Not in Tauri (browser dev) — open in a new tab as a fallback.
    window.open(url, "_blank", "noopener");
  }
}

export function CloudPanel({ project, onTranscribed }: Props) {
  const t = useT();
  const providersQuery = useQuery({
    queryKey: ["cloud-providers"],
    queryFn: () => ipc.asr.cloudProviders(),
  });
  const secretsQuery = useQuery({
    queryKey: ["secret-status"],
    queryFn: () => ipc.secrets.status(),
  });

  const providers = providersQuery.data ?? [];
  const minutes = project.video_duration_ms / 60_000;

  const [selected, setSelected] = useState<CloudProvider | null>(null);
  const selectedInfo = providers.find((p) => p.provider === selected) ?? null;
  const [consentFor, setConsentFor] = useState<CloudProviderInfo | null>(null);
  const [transcribing, setTranscribing] = useState(false);
  const [transcribeErr, setTranscribeErr] = useState<string | null>(null);

  const keySet = (p: CloudProvider) =>
    secretsQuery.data?.find((s) => s.provider === KEY_FOR[p])?.present ?? false;

  async function doTranscribe(provider: CloudProvider) {
    setTranscribing(true);
    setTranscribeErr(null);
    try {
      const captions = await ipc.asr.cloudTranscribe(project, provider);
      onTranscribed(captions);
    } catch (e) {
      setTranscribeErr(
        e instanceof IPCError
          ? e.message
          : t("cloudTranscribeFailed", { error: String(e) }),
      );
    } finally {
      setTranscribing(false);
    }
  }

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-1 flex items-center gap-2">
        <Cloud size={18} className="text-[var(--color-accent-400)]" />
        <h2 className="text-[var(--text-ui-lg)] font-semibold">
          {t("cloudTitle")}
        </h2>
      </div>
      <p className="mb-4 text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
        {t("cloudIntro")}
      </p>

      <ul className="space-y-2">
        {providers.map((p) => {
          const cost = minutes * p.price_per_min_usd;
          const hasKey = keySet(p.provider);
          const isSelected = selected === p.provider;
          return (
            <li
              key={p.provider}
              className={cn(
                "rounded-lg border p-3",
                isSelected
                  ? "border-[var(--color-accent-500)] bg-[var(--color-accent-500)]/8"
                  : "border-[var(--color-border)]",
              )}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-[var(--text-ui-sm)] font-semibold">
                      {p.display_name}
                    </span>
                    <span className="text-[10px] text-[var(--color-fg-subtle)]">
                      ~${p.price_per_min_usd.toFixed(4)}/min
                    </span>
                    {p.max_upload_bytes != null && (
                      <span className="text-[10px] text-[var(--color-fg-subtle)]">
                        {t("cloudMaxUpload", {
                          size: `${Math.round(p.max_upload_bytes / (1024 * 1024))} MB`,
                        })}
                      </span>
                    )}
                    {!p.word_confidence && (
                      <span
                        className="text-[10px] text-[var(--color-warning)]"
                        title={t("cloudSegmentOnlyTitle")}
                      >
                        {t("cloudSegmentOnly")}
                      </span>
                    )}
                    {hasKey ? (
                      <span className="flex items-center gap-1 text-[10px] text-[var(--color-success)]">
                        <KeyRound size={9} /> {t("cloudKeySet")}
                      </span>
                    ) : (
                      <span className="flex items-center gap-1 text-[10px] text-[var(--color-fg-subtle)]">
                        <KeyRound size={9} /> {t("cloudKeyMissing")}
                      </span>
                    )}
                  </div>
                  <div className="mt-1 flex items-center gap-3 text-[var(--text-ui-xs)] text-[var(--color-fg-muted)]">
                    <span>
                      {t("cloudEstimatedLabel")}{" "}
                      <strong>${cost.toFixed(2)}</strong>{" "}
                      {t("cloudForMinutes", { minutes: minutes.toFixed(1) })}
                    </span>
                    <button
                      type="button"
                      onClick={() => openPrivacy(p.privacy_url)}
                      className="flex items-center gap-1 underline-offset-2 hover:text-[var(--color-accent-400)] hover:underline"
                    >
                      {t("cloudPrivacy")} <ExternalLink size={10} />
                    </button>
                  </div>
                </div>
                {isSelected ? (
                  <span className="flex items-center gap-1 text-[var(--text-ui-xs)] font-semibold text-[var(--color-accent-300)]">
                    <Check size={13} /> {t("cloudSelected")}
                  </span>
                ) : (
                  <button
                    type="button"
                    onClick={() => setConsentFor(p)}
                    className="shrink-0 rounded-md border border-[var(--color-border)] px-3 py-1 text-[var(--text-ui-xs)] font-medium hover:border-[var(--color-accent-600)]"
                  >
                    {t("cloudSelect")}
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>

      {selectedInfo && (
        <div className="mt-4 rounded-lg border border-[var(--color-accent-600)]/40 bg-[var(--color-accent-500)]/5 p-3">
          {keySet(selectedInfo.provider) ? (
            <>
              <button
                type="button"
                onClick={() => doTranscribe(selectedInfo.provider)}
                disabled={transcribing}
                className="flex items-center gap-2 rounded-lg bg-[var(--color-accent-600)] px-4 py-2 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)] disabled:opacity-50"
              >
                {transcribing ? (
                  <Loader2 size={15} className="animate-spin" />
                ) : (
                  <Cloud size={15} />
                )}
                {transcribing
                  ? t("cloudTranscribing")
                  : t("cloudTranscribeWith", {
                      provider: selectedInfo.display_name,
                    })}
              </button>
              <p className="mt-2 text-[10px] text-[var(--color-fg-subtle)]">
                {t("cloudUploadHint", { provider: selectedInfo.display_name })}
                {selectedInfo.provider === "assembly-ai" &&
                  t("cloudAssemblyNote")}
              </p>
              {transcribeErr && (
                <p className="mt-2 text-[var(--text-ui-sm)] text-[var(--color-danger)]">
                  {transcribeErr}
                </p>
              )}
            </>
          ) : (
            <p className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              {t("cloudNeedKey", { provider: selectedInfo.display_name })}
            </p>
          )}
        </div>
      )}

      {consentFor && (
        <ConsentDialog
          info={consentFor}
          onCancel={() => setConsentFor(null)}
          onAccept={() => {
            setSelected(consentFor.provider);
            setConsentFor(null);
          }}
        />
      )}
    </div>
  );
}

function ConsentDialog({
  info,
  onAccept,
  onCancel,
}: {
  info: CloudProviderInfo;
  onAccept: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/50 p-4">
      <div className="w-full max-w-md rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] p-5 shadow-xl">
        <div className="mb-2 flex items-center gap-2">
          <ShieldAlert size={18} className="text-[var(--color-warning)]" />
          <h3 className="text-[var(--text-ui-md)] font-semibold">
            {t("consentTitle", { provider: info.display_name })}
          </h3>
        </div>
        <p className="text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
          {info.consent_text}
        </p>
        <button
          type="button"
          onClick={() => openPrivacy(info.privacy_url)}
          className="mt-2 flex items-center gap-1 text-[var(--text-ui-sm)] text-[var(--color-accent-400)] underline-offset-2 hover:underline"
        >
          {t("consentReadPrivacy")} <ExternalLink size={11} />
        </button>
        <div className="mt-5 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md px-3 py-1.5 text-[var(--text-ui-sm)] hover:bg-[var(--color-bg-surface)]"
          >
            {t("actionCancel")}
          </button>
          <button
            type="button"
            onClick={onAccept}
            className="rounded-md bg-[var(--color-accent-600)] px-4 py-1.5 text-[var(--text-ui-sm)] font-semibold text-[var(--color-neutral-950)] hover:bg-[var(--color-accent-500)]"
          >
            {t("consentAccept")}
          </button>
        </div>
      </div>
    </div>
  );
}
