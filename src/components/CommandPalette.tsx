import { useEffect, useMemo, useRef, useState } from "react";
import { Command, Search, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";

/** A single command. `group` clusters related commands in the list. */
export interface PaletteCommand {
  id: string;
  label: string;
  group: string;
  icon: LucideIcon;
  run: () => void;
  keywords?: string;
}

/**
 * App-wide ⌘K command palette for the dense SundayEdit workspace — one search
 * box that reaches every dock tool, pipeline modal and project action, so the
 * 9-tool rail and the top bar aren't the only way in. A small floating ⌘K
 * trigger keeps it discoverable.
 */
export function CommandPalette({ commands }: { commands: PaletteCommand[] }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      } else if (e.key === "Escape") {
        setOpen(false);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActive(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) =>
      (c.label + " " + (c.keywords ?? "") + " " + c.group)
        .toLowerCase()
        .includes(q),
    );
  }, [commands, query]);

  useEffect(() => {
    if (active >= filtered.length) setActive(0);
  }, [filtered.length, active]);

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label="Open command palette"
        className="fixed bottom-4 right-4 z-40 flex items-center gap-2 rounded-full border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/90 px-3.5 py-2 text-[var(--text-ui-xs)] text-[var(--color-fg-muted)] shadow-lg backdrop-blur transition-colors hover:text-[var(--color-fg)]"
      >
        <Command size={14} aria-hidden />
        <span className="font-mono">⌘K</span>
      </button>
    );
  }

  function runAt(index: number) {
    const cmd = filtered[index];
    if (!cmd) return;
    setOpen(false);
    cmd.run();
  }

  const groups: {
    name: string;
    items: { cmd: PaletteCommand; index: number }[];
  }[] = [];
  filtered.forEach((cmd, index) => {
    let g = groups.find((x) => x.name === cmd.group);
    if (!g) {
      g = { name: cmd.group, items: [] };
      groups.push(g);
    }
    g.items.push({ cmd, index });
  });

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[16vh] backdrop-blur-sm"
      onMouseDown={() => setOpen(false)}
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div
        className="w-full max-w-lg overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5 border-b border-[var(--color-border)] px-4 py-3">
          <Search
            size={16}
            className="text-[var(--color-fg-muted)]"
            aria-hidden
          />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(a + 1, filtered.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                runAt(active);
              }
            }}
            placeholder="Search commands…"
            className="w-full bg-transparent text-[var(--text-ui-sm)] text-[var(--color-fg)] outline-none placeholder:text-[var(--color-fg-subtle)]"
          />
        </div>
        <div className="max-h-[50vh] overflow-y-auto py-2">
          {filtered.length === 0 ? (
            <div className="px-4 py-6 text-center text-[var(--text-ui-sm)] text-[var(--color-fg-muted)]">
              No matching commands.
            </div>
          ) : (
            groups.map((g) => (
              <div key={g.name} className="mb-1">
                <div className="px-4 pb-1 pt-2 text-[0.65rem] font-medium uppercase tracking-wider text-[var(--color-fg-subtle)]">
                  {g.name}
                </div>
                {g.items.map(({ cmd, index }) => {
                  const Icon = cmd.icon;
                  return (
                    <button
                      key={cmd.id}
                      type="button"
                      onMouseEnter={() => setActive(index)}
                      onClick={() => runAt(index)}
                      className={cn(
                        "flex w-full items-center gap-3 px-4 py-2 text-left text-[var(--text-ui-sm)] transition-colors",
                        index === active
                          ? "bg-[var(--color-accent-600)]/20 text-[var(--color-fg)]"
                          : "text-[var(--color-fg-muted)] hover:text-[var(--color-fg)]",
                      )}
                    >
                      <Icon
                        size={16}
                        strokeWidth={1.75}
                        aria-hidden
                        className="shrink-0"
                      />
                      {cmd.label}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
