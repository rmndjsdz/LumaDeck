import { useEffect } from "react";
import { useNavigationStore } from "../../stores/navigation-store";

const HIDE_DELAY_MS = 550;

export function AutoCursor() {
  const inputMode = useNavigationStore((state) => state.inputMode);

  useEffect(() => {
    const root = document.documentElement;
    let timer: number | null = null;
    const show = () => {
      root.dataset.cursor = "visible";
      if (timer !== null) window.clearTimeout(timer);
      if (inputMode !== "mouse") {
        timer = window.setTimeout(() => {
          root.dataset.cursor = "hidden";
          timer = null;
        }, HIDE_DELAY_MS);
      }
    };
    const handlePointerMove = () => show();
    root.dataset.cursor = inputMode === "mouse" ? "visible" : "hidden";
    window.addEventListener("pointermove", handlePointerMove, {
      passive: true,
    });
    if (inputMode !== "mouse") show();
    return () => {
      if (timer !== null) window.clearTimeout(timer);
      window.removeEventListener("pointermove", handlePointerMove);
      delete root.dataset.cursor;
    };
  }, [inputMode]);

  return null;
}
