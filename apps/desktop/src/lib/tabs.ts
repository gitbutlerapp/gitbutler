import type { Writable } from 'svelte/store';

export interface TabContext {
	selectedIndex: Writable<string>;
	setSelected: (id: string) => Writable<string>;
}
