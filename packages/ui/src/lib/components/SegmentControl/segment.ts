import type { Writable } from 'svelte/store';

export interface SegmentItem {
	id: string;
	index: number;
	disabled: boolean;
}
export interface SegmentContext {
	focusedSegmentIndex: Writable<number>;
	selectedSegmentIndex: Writable<number>;
	length: Writable<number>;
	setIndex(): number;
	addSegment(segment: SegmentItem): void;
	setSelected(index: number): void;
}
