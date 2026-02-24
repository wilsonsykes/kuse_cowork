import { createSignal, onMount } from "solid-js";
import { getVersion } from "@tauri-apps/api/app";

export const useAppVersion = () => {
  const [version, setVersion] = createSignal(`v${__APP_VERSION__}`);

  onMount(async () => {
    try {
      const tauriVersion = await getVersion();
      if (tauriVersion) {
        setVersion(`v${tauriVersion}`);
      }
    } catch {
      // Keep Vite-injected fallback version in browser-only dev contexts.
    }
  });

  return version;
};
