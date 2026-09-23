import preview from "#storybook/preview";
import { DiffStats } from "./DiffStats.tsx";

const meta = preview.meta({
	component: DiffStats,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2134-5921",
		},
	},
	args: {
		added: 364,
		removed: 20,
	},
});

export const Default = meta.story({
	args: {
		added: 364,
		removed: 20,
	},
});

export const OnlyAdditions = meta.story({
	args: {
		added: 6,
		removed: 0,
	},
});

export const OnlyDeletions = meta.story({
	args: {
		added: 0,
		removed: 12,
	},
});
