import { previewConfig } from "@gitbutler/ui-react/storybook/shared.ts";
import { definePreview } from "@storybook/react-vite";

import "../ui/src/global.css";
import "@gitbutler/ui-react/storybook/storybook-styles.css";

// Provide a minimal window.lite stub so hotkeys.ts doesn't crash in Storybook, and so a
// copy button copies for real rather than throwing.
(
	window as unknown as {
		lite?: { platform: string; clipboardWriteText: (text: string) => Promise<void> };
	}
).lite ??= {
	platform: navigator.platform.toLowerCase().includes("mac") ? "darwin" : "linux",
	clipboardWriteText: (text) => navigator.clipboard.writeText(text),
};

export default definePreview(previewConfig);
