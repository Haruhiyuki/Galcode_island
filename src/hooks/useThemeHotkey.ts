import { useEffect } from "react";
import { useAppStore } from "../stores/useAppStore";

export function useThemeHotkey(): void {
  const toggleTheme = useAppStore((state) => state.toggleTheme);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const isToggleShortcut =
        (event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === "l";

      if (!isToggleShortcut) {
        return;
      }

      event.preventDefault();
      toggleTheme();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [toggleTheme]);
}
