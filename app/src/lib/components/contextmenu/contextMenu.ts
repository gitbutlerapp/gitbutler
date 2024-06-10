import type { Writable } from 'svelte/store';

export type ContextMenuType = 'checklist' | 'select' | 'normal';

export interface ContextMenuItem {
	id: string;
	label: string;
}
export interface ContextMenuContext {
	type: ContextMenuType;
	selection: Writable<ContextMenuItem | undefined>;
}
