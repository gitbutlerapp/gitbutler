import preview from "#storybook/preview";

import { DiffStat } from "./DiffStat.tsx";

const meta = preview.meta({
	component: DiffStat,
});

export const Default = meta.story({
	args: {
		filesChanged: 3,
		linesAdded: 42,
		linesRemoved: 17,
	},
});

export const SingleFile = Default.extend({
	args: {
		filesChanged: 1,
		linesAdded: 5,
		linesRemoved: 2,
	},
});

export const AdditionsOnly = Default.extend({
	args: {
		filesChanged: 2,
		linesAdded: 100,
		linesRemoved: 0,
	},
});

export const DeletionsOnly = Default.extend({
	args: {
		filesChanged: 1,
		linesAdded: 0,
		linesRemoved: 50,
	},
});
