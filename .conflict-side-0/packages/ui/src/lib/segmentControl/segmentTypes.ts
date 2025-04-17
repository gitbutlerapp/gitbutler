import type { Writable } from 'svelte/store';

export interface SegmentItem {
	index: number;
}
export interface SegmentContext {
	selectedSegmentIndex: Writable<number>;
	setIndex(): number;
	addSegment(segment: SegmentItem): void;
	setSelected({ index, id }: { index: number; id: string }): void;
}
