import type { Writable } from 'svelte/store';

export enum TabStyle {
	SegmentControl = 'segment-control',
	Custom = 'custom'
}

export interface TabContext {
	style: TabStyle;
	selectedIndex: Writable<string>;
	setSelected: (id: string) => Writable<string>;
}
