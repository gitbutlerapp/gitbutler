import type { Session, Delta } from '$lib/api';

export type UISession = {
	session: Session;
	deltas: Record<string, Delta[]>;
	earliestDeltaTimestampMs: number;
	latestDeltaTimestampMs: number;
};
