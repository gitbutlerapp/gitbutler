import type { Delta } from '$lib/backend/deltas';

export type Frame = {
	sessionId: string;
	doc: string;
	deltas: Delta[];
	filepath: string;
};
