import preview from "#storybook/preview";

import { Keys } from "./Keys.tsx";

const meta = preview.meta({
	component: Keys,
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
