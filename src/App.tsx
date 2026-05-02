import { useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { GlobalTopBar } from "./components/GlobalTopBar";
import { MainView } from "./components/MainView";
import { WelcomeView } from "./components/welcome/WelcomeView";
import { useAgentIPC } from "./hooks/useAgentIPC";
import { useThemeHotkey } from "./hooks/useThemeHotkey";
import { useAppStore } from "./stores/useAppStore";

function App(): JSX.Element {
  const isStarted = useAppStore((state) => state.isStarted);

  useThemeHotkey();
  useAgentIPC();

  const currentScreen = useMemo(() => {
    return isStarted ? <MainView /> : <WelcomeView />;
  }, [isStarted]);

  return (
    <main className="relative h-screen w-screen overflow-hidden bg-transparent text-zinc-900 transition-colors dark:text-zinc-100">
      <div className="absolute inset-2 overflow-hidden rounded-[22px] border border-black/10 bg-[#f5efdf] shadow-[0_20px_60px_rgba(0,0,0,0.22)] dark:border-white/10 dark:bg-[#2f3338]">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.45),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(0,0,0,0.08),transparent_30%)] dark:bg-[radial-gradient(circle_at_12%_18%,rgba(255,255,255,0.07),transparent_38%),radial-gradient(circle_at_88%_82%,rgba(255,255,255,0.04),transparent_30%)]" />
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
      </div>
    </main>
  );
}

export default App;
