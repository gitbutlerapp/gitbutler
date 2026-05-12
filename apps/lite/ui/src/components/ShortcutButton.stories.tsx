import preview from "#storybook/preview";
import { Tooltip } from "@base-ui/react";

import { ShortcutButton } from "./ShortcutButton.tsx";

const meta = preview.meta({
	component: ShortcutButton,
	decorators: (Story) => (
		<Tooltip.Provider>
			<Story />
		</Tooltip.Provider>
	),
});

export const Default = meta.story({
	args: {
		children: "Hover me",
		hotkeys: ["A"],
	},
});

export const Modifier = Default.extend({
	args: {
		hotkeys: ["Mod+A"],
	},
});
