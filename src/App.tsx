import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo } from "react";
import { GlobalTopBar } from "./components/GlobalTopBar";
import { MainView } from "./components/MainView";
import { WelcomeView } from "./components/welcome/WelcomeView";
import { SettingsModal } from "./components/settings/SettingsModal";
import { useAgentIPC } from "./hooks/useAgentIPC";
import { useThemeHotkey } from "./hooks/useThemeHotkey";
import { useAppStore } from "./stores/useAppStore";
import { useSettingsStore } from "./stores/useSettingsStore";

function App(): JSX.Element {
  const isStarted = useAppStore((state) => state.isStarted);

  useThemeHotkey();
  useAgentIPC();

  useEffect(() => {
    const state = useSettingsStore.getState();
    invoke("update_llm_settings", {
      baseUrl: state.apiBaseUrl,
      apiKey: state.apiKey,
      nickname: state.nickname,
      systemPrompt: state.systemPrompt,
    }).catch(console.error);
  }, []);

  const currentScreen = useMemo(() => {
    return isStarted ? <MainView /> : <WelcomeView />;
  }, [isStarted]);

  return (
    <main className="box-border flex min-h-screen w-full flex-1 flex-col bg-[color:var(--app-bg)] p-2 text-zinc-900 transition-colors dark:text-zinc-100">
      <div className="relative flex min-h-0 min-h-[560px] flex-1 flex-col overflow-hidden rounded-[22px] border border-black/10 bg-[#f5efdf] shadow-[0_20px_60px_rgba(0,0,0,0.22)] dark:border-white/10 dark:bg-[#2f3338]">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.45),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(0,0,0,0.08),transparent_30%)] dark:bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.07),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(255,255,255,0.04),transparent_30%)]" />
        <GlobalTopBar />
        <div
          key={isStarted ? "main" : "welcome"}
          className="relative z-10 flex min-h-0 flex-1 flex-col overflow-hidden pt-8"
        >
          {currentScreen}
        </div>
        <SettingsModal />
      </div>
    </main>
  );
}

export default App;
