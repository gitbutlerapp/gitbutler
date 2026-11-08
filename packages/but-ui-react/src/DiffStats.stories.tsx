import { stories } from "../docs/stories.ts";
import { DiffStats } from "./DiffStats.tsx";

const meta = stories({
	title: "Content & status/DiffStats",
	id: "components-diffstats",
	component: DiffStats,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2134-5921",
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
