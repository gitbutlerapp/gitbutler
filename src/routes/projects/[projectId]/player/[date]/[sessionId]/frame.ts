import type { Delta } from '$lib/api';

export type Frame = {
	sessionId: string;
	doc: string;
	deltas: Delta[];
	filepath: string;
};
