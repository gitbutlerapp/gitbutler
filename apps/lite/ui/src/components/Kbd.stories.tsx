import preview from "#storybook/preview";

import { Kbd } from "./Kbd.tsx";

const meta = preview.meta({
	component: Kbd,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=426-883&t=zrLHwmcDOZvnbONB-1",
		},
	},
});

export const Default = meta.story({
	args: {
		hotkey: "A",
	},
});

export const Modifier = meta.story({
	args: {
		hotkey: "Mod+A",
	},
});

export const Sequence = meta.story({
	args: {
		hotkey: ["G", "F"],
	},
});
