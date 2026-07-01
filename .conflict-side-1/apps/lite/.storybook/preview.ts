import type { Decorator } from "@storybook/react-vite";
import { definePreview } from "@storybook/react-vite";

import "../ui/src/global.css";
import "./storybook-styles.css";

const themeDecorator: Decorator = (Story, context) => {
	const globals = context.globals as Record<string, string>;
	const theme = globals["theme"] ?? "light";
	document.documentElement.classList.toggle("dark", theme === "dark");
	document.documentElement.classList.toggle("light", theme !== "dark");
	return Story();
};

export default definePreview({
	addons: [],
	parameters: {
		docs: {
			codePanel: true,
		},
	},
	initialGlobals: {
		theme: "light",
	} as never,
	globalTypes: {
		theme: {
			name: "Theme",
			description: "Toggle between light and dark theme",
			toolbar: {
				icon: "contrast",
				items: [
					{ value: "light", title: "Light mode", icon: "sun" },
					{ value: "dark", title: "Dark mode", icon: "moon" },
				],
				showName: false,
				dynamicTitle: true,
			},
		},
	} as never,
	decorators: [themeDecorator] as never,
});
