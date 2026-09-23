import preview from "#storybook/preview";
import { ChangeScale } from "./ChangeScale.tsx";

const meta = preview.meta({
	component: ChangeScale,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2146-1562",
		},
	},
	args: {
		added: 34,
		removed: 28,
	},
});

export const Default = meta.story({});

export const MostlyAdditions = meta.story({
	args: {
		added: 364,
		removed: 20,
	},
});

/** One addition among many deletions still keeps a square. */
export const BarelyAnyAdditions = meta.story({
	args: {
		added: 1,
		removed: 900,
	},
});

export const OnlyDeletions = meta.story({
	args: {
		added: 0,
		removed: 12,
	},
});
