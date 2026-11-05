import { cn } from "@anotai/ui";
import {
  BookA,
  History as HistoryIcon,
  Home as HomeIcon,
  Settings as SettingsIcon,
} from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";
import { api, onCoreEvent, type Theme } from "./lib/core";
import { useCoreStore } from "./lib/store";
import { Dictionary } from "./views/Dictionary";
import { History } from "./views/History";
import { Home } from "./views/Home";
import { Onboarding } from "./views/Onboarding";
import { SettingsView } from "./views/Settings";

type Tab = "home" | "history" | "dictionary" | "settings";

const tabs: { id: Tab; label: string; icon: ReactNode }[] = [
  { id: "home", label: "Início", icon: <HomeIcon /> },
  { id: "history", label: "Histórico", icon: <HistoryIcon /> },
  { id: "dictionary", label: "Dicionário", icon: <BookA /> },
  { id: "settings", label: "Configurações", icon: <SettingsIcon /> },
];

const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");

/** Aplica o tema escolhido (ou o do sistema). */
export function applyTheme(theme: Theme) {
  const dark = theme === "dark" || (theme === "system" && prefersDark.matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function App() {
  const apply = useCoreStore((s) => s.apply);
  const [tab, setTab] = useState<Tab>("home");
  const [onboarding, setOnboarding] = useState<boolean | null>(null);

  useEffect(() => {
    api
      .getSettings()
      .then((s) => setOnboarding(!s.onboardingDone))
      .catch(() => setOnboarding(false));
  }, []);

  useEffect(() => {
    const unlisten = onCoreEvent(apply);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [apply]);

  if (onboarding === null) return null;
  if (onboarding) {
    return (
      <Onboarding
        onDone={() => {
          setOnboarding(false);
          setTab("home");
        }}
      />
    );
  }

  return (
    <div className="flex min-h-screen">
      <nav
        aria-label="Seções"
        className="flex w-52 shrink-0 flex-col gap-1 border-border border-r bg-muted/40 p-3"
      >
        <div className="mb-4 flex items-center gap-2 px-2 pt-1">
          <img src="/anotai-simbolo.svg" alt="" className="size-8" />
          <span className="font-display font-semibold text-lg">AnotAI</span>
        </div>
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            aria-current={tab === t.id ? "page" : undefined}
            className={cn(
              "flex items-center gap-2.5 rounded-md px-3 py-2 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&_svg]:size-4",
              tab === t.id
                ? "bg-background font-medium text-foreground shadow-sm"
                : "text-muted-foreground hover:bg-background/60 hover:text-foreground",
            )}
          >
            {t.icon}
            {t.label}
          </button>
        ))}
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto px-8 py-8">
        <div className="mx-auto max-w-3xl">
          {tab === "home" && <Home onOpenSettings={() => setTab("settings")} />}
          {tab === "history" && <History />}
          {tab === "dictionary" && <Dictionary />}
          {tab === "settings" && <SettingsView />}
        </div>
      </main>
    </div>
  );
}
