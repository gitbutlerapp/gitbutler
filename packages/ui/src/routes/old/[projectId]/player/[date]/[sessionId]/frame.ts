import type { Delta } from '$lib/api/ipc/deltas';

export type Frame = {
	sessionId: string;
	doc: string;
	deltas: Delta[];
	filepath: string;
};
