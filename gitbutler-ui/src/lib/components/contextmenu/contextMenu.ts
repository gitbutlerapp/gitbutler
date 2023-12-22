import type { BehaviorSubject } from 'rxjs';

export type ContextMenuType = 'checklist' | 'select' | 'normal';

export interface ContextMenuItem {
	id: string;
	label: string;
}
export interface ContextMenuContext {
	type: ContextMenuType;
	selection$: BehaviorSubject<ContextMenuItem | undefined>;
}
