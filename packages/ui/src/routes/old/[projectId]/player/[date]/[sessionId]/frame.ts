import type { Delta } from '$lib/api/deltas';

export type Frame = {
	sessionId: string;
	doc: string;
	deltas: Delta[];
	filepath: string;
};
