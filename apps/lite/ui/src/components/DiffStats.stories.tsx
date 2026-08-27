import preview from "#storybook/preview";
import { DiffStats } from "./DiffStats.tsx";

const meta = preview.meta({
	component: DiffStats,
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
