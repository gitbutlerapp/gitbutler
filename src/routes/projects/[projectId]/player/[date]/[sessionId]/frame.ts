import type { Delta } from '$lib/api';

export type Frame = {
	doc: string;
	deltas: Delta[];
	filepath: string;
};
