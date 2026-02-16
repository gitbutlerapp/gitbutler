import type { Writable } from 'svelte/store';

export interface SegmentContext {
	selectedSegmentId: Writable<string | undefined>;
	registerSegment(id: string): void;
	selectSegment(id: string): void;
}
