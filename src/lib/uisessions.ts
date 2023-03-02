import type { Session } from '$lib/sessions';
import type { Delta } from '$lib/deltas';

export type UISession = {
	session: Session;
	deltas: Record<string, Delta[]>;
	earliestDeltaTimestampMs: number;
	latestDeltaTimestampMs: number;
};
