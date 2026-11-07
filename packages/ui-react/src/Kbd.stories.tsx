import { stories } from "../docs/stories.ts";
import { Kbd } from "./Kbd.tsx";

const meta = stories({
	title: "Actions/Kbd",
	id: "components-kbd",
	component: Kbd,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=426-883&t=zrLHwmcDOZvnbONB-1",
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
