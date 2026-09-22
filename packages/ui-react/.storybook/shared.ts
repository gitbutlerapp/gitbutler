import type { Decorator } from "@storybook/react-vite";

const themeDecorator: Decorator = (Story, context) => {
	const globals = context.globals as Record<string, string>;
	const theme = globals["theme"] ?? "light";
	document.documentElement.classList.toggle("dark", theme === "dark");
	document.documentElement.classList.toggle("light", theme !== "dark");
	return Story();
};

/**
 * What every Storybook showing these components configures: the theme toolbar
 * and the decorator behind it. Storybook reads a preview file's source, not
 * only what it exports, and wants to find the `definePreview` call there, so a
 * host can't re-export this package's preview. It calls `definePreview` with
 * this instead.
 */
export const previewConfig = {
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
};
