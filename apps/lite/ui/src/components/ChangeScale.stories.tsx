import preview from "#storybook/preview";
import { ChangeScale } from "./ChangeScale.tsx";

const meta = preview.meta({
	component: ChangeScale,
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
