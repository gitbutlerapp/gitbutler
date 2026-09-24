import { previewConfig } from "@gitbutler/ui-react/storybook/shared.ts";
import addonA11y from "@storybook/addon-a11y";
import addonDocs from "@storybook/addon-docs";
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

// Addons that change what the preview renders are registered here as well as in main.ts:
// docs for docs pages such as the design notes, a11y for the checks on every story.
export default definePreview({ ...previewConfig, addons: [addonDocs(), addonA11y()] });
