import addonA11y from "@storybook/addon-a11y";
import addonDocs from "@storybook/addon-docs";
import { definePreview } from "@storybook/react-vite";
import { previewConfig } from "./shared.ts";

import "./storybook-styles.css";

// Addons that change what the preview renders are registered here as well as in main.ts:
// docs for docs pages such as the design notes, a11y for the checks on every story.
export default definePreview({
	...previewConfig,
	parameters: {
		...previewConfig.parameters,
		// axe stays in the Accessibility tab; story tests skip it for now.
		a11y: { test: "off" },
	},
	addons: [addonDocs(), addonA11y()],
});
