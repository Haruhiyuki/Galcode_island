import { AnimatePresence, motion } from "framer-motion";
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
    <main className="relative h-screen w-screen overflow-hidden bg-slate-50 text-zinc-900 transition-colors dark:bg-[#0B1120] dark:text-zinc-100">
      {/* Dynamic diffused light background */}
      <motion.div
        className="pointer-events-none absolute inset-0"
        aria-hidden="true"
      >
        {/* Sky blue blob — drifts slowly top-left */}
        <motion.div
          className="absolute -top-1/4 -left-1/4 h-[60%] w-[60%] rounded-full bg-sky-200/30 blur-3xl dark:bg-sky-400/15"
          animate={{
            x: [0, 30, -20, 15, 0],
            y: [0, -20, 25, -10, 0],
            scale: [1, 1.08, 0.95, 1.04, 1],
          }}
          transition={{
            duration: 18,
            repeat: Infinity,
            ease: "easeInOut",
          }}
        />
        {/* Orange-yellow accent blob — drifts slowly bottom-right */}
        <motion.div
          className="absolute -bottom-1/4 -right-1/4 h-[50%] w-[50%] rounded-full bg-amber-200/25 blur-3xl dark:bg-amber-400/10"
          animate={{
            x: [0, -25, 15, -10, 0],
            y: [0, 20, -30, 15, 0],
            scale: [1, 0.96, 1.06, 0.98, 1],
          }}
          transition={{
            duration: 22,
            repeat: Infinity,
            ease: "easeInOut",
          }}
        />
        {/* Secondary sky blue blob — center-right, larger, very subtle */}
        <motion.div
          className="absolute top-1/3 -right-1/6 h-[55%] w-[55%] rounded-full bg-sky-300/15 blur-3xl dark:bg-sky-500/8"
          animate={{
            x: [0, -18, 22, -8, 0],
            y: [0, 15, -12, 20, 0],
            scale: [1, 1.05, 0.97, 1.03, 1],
          }}
          transition={{
            duration: 20,
            repeat: Infinity,
            ease: "easeInOut",
          }}
        />
      </motion.div>

      {/* Glass container */}
      <div className="absolute inset-2 overflow-hidden rounded-[22px] border border-white/60 bg-white/70 shadow-[0_8px_40px_rgba(0,0,0,0.06)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60 dark:shadow-none">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.3),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(0,0,0,0.04),transparent_30%)] dark:bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.04),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(255,255,255,0.02),transparent_30%)]" />
        <GlobalTopBar />
        <AnimatePresence mode="wait">
          <motion.div
            key={isStarted ? "main" : "welcome"}
            initial={{ opacity: 0, scale: 0.985 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 0.985 }}
            transition={{ duration: 0.25, ease: "easeOut" }}
            className="relative z-10 h-full w-full pt-8"
          >
            {currentScreen}
          </motion.div>
        </AnimatePresence>
        <SettingsModal />
      </div>
    </main>
  );
}

export default App;
