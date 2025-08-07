import { InjectionToken } from '@gitbutler/shared/context';
import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
import { type Reactive } from '@gitbutler/shared/storeUtils';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { on } from 'svelte/events';

export const FOCUS_MANAGER = new InjectionToken<FocusManager>('FocusManager');

export enum DefinedFocusable {
	MainViewport = 'workspace',
	ViewportLeft = 'workspace-left',
	ViewportRight = 'workspace-right',
	ViewportDrawerRight = 'workspace-drawer-right',
	ViewportMiddle = 'workspace-middle',
	UncommittedChanges = 'uncommitted-changes',
	Drawer = 'drawer',
	Branches = 'branches',
	Stack = 'stack',
	Preview = 'preview',
	// Only one of these can be in the dom at any given time.
	ChangedFiles = 'changed-files'
}

export function stackFocusableId(stackId: string) {
	return `${DefinedFocusable.Stack}:${stackId}`;
}

export function uncommittedFocusableId(stackId?: string) {
	return `${DefinedFocusable.UncommittedChanges}:${stackId}`;
}

/**
 * If the provided ID is a assigned-changes id, it will return the stackId as a
 * string, otherwise, it will return undefined.
 */
export function parseFocusableId(id: string): string | undefined {
	const halves = id.split(':');
	if (halves.length !== 2) {
		return;
	}
	return halves[1];
}

export type Focusable = DefinedFocusable | string;

export type FocusableElement = {
	key: Focusable;
	parentId: Focusable | null;
	element: HTMLElement;
	children: Focusable[];
};

/**
 * Manages focusable areas through the `focusable` svelte action.
 *
 * As each action registers with this class we build up a tree of elements,
 * and when the user clicks any item on the page we iterate over the parents
 * until we find a match in the lookup map. The id of this focusable
 * area is then stored as the current focused element of the app.
 *
 * The purpose of this class is two fold:
 * - updates to the UI when the focused element changes
 * - support keyboard navigation and keyboard actions
 *
 * TODO: Should we have a stronger type for the id?
 *
 * @example
 * <div use:focusable={{ id: 'parent' }}>
 *     <div use:focusable={{ id: 'child', parentId: 'parent' }}>...</div>
 * </div>
 */
export class FocusManager implements Reactive<Focusable | undefined> {
	/** Elements registered using `focusable.ts` svelte action. */
	private elements: FocusableElement[] = [];

	/** Element to focusable lookup. */
	private lookup = new Map<HTMLElement, FocusableElement>();

	/** The id of the most recently focused item. */
	private _current: Focusable | undefined = $state();

	private handleMouse = this.handleClick.bind(this);
	private handleKeys = this.handleKeydown.bind(this);

	constructor() {
		$effect(() => {
			// We listen for events on the document in the bubble phase, giving
			// other event handlers an opportunity to stop propagation.
			return mergeUnlisten(
				on(document, 'click', this.handleMouse, { capture: true }),
				on(document, 'keypress', this.handleKeys)
			);
		});
	}

	get current() {
		return this._current;
	}

	handleClick(e: Event) {
		if (e.target instanceof HTMLElement) {
			let pointer: HTMLElement | null = e.target;
			while (pointer) {
				const item = this.lookup.get(pointer);
				if (item) {
					this.setActive(item.key);
					break;
				}
				pointer = pointer.parentElement;
			}
		}
	}

	register(id: Focusable, parentId: Focusable | null, element: HTMLElement) {
		let item = this.elements.find((area) => area.key === id);
		if (item) {
			this.lookup.delete(element);
			item.element = element;
			item.parentId = parentId;
		} else {
			item = { key: id, parentId, element, children: [] };
			this.elements.push(item);
		}

		this.lookup.set(element, item);

		const parent = this.elements.find((a) => a.key === parentId);
		if (parent) parent.children.push(id);

		item.children.push(...this.elements.filter((a) => id === a.parentId).map((a) => a.key));
	}

	unregister(id: Focusable) {
		const index = this.elements.findIndex((area) => area.key === id);
		if (index !== -1) {
			const area = this.elements[index]!;
			this.lookup.delete(area.element);

			// Remove from parent's children
			const parent = this.elements.find((a) => a.key === area.parentId);
			if (parent) {
				parent.children = parent.children.filter((childId) => childId !== id);
			}

			this.elements.splice(index, 1);
		}

		if (this._current === id) {
			this._current = undefined;
		}
	}

	setActive(id: Focusable) {
		this._current = id;
	}

	focusSibling(forward = true) {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.key === currentId);
		if (!area || !area.parentId) return;

		const parent = this.elements.find((a) => a.key === area.parentId);
		if (!parent) return;

		const siblings = parent.children;
		const index = siblings.indexOf(currentId);
		const nextIndex = (index + (forward ? 1 : siblings.length - 1)) % siblings.length;

		this.setActive(siblings[nextIndex]!);
		this.elements.find((a) => a.key === siblings[nextIndex])?.element.focus();
	}

	focusParent() {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.key === currentId);
		if (area?.parentId) {
			this.setActive(area.parentId);
			this.elements.find((a) => a.key === area.parentId)?.element.focus();
		}
	}

	focusFirstChild() {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.key === currentId);
		if (area && area.children.length > 0) {
			const firstChild = area.children[0];
			this.setActive(firstChild!);
			this.elements.find((a) => a.key === firstChild)?.element.focus();
		}
	}

	handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Tab') {
			event.preventDefault();
			this.focusSibling(!event.shiftKey);
		} else if (event.metaKey && event.key === 'ArrowUp') {
			event.preventDefault();
			this.focusParent();
		} else if (event.metaKey && event.key === 'ArrowDown') {
			event.preventDefault();
			this.focusFirstChild();
		}
	}

	/**
	 * Works like an html radio input group.
	 *
	 * This function takes N focusable enums and remembers which was last
	 * active. The idea is that when you e.g. click the uncommitted focus
	 * area then your file preview switches to that selection, and any
	 * selected items are highlighted in blue. This focus only changes
	 * when another one of the triggers gets activated.
	 *
	 */
	radioGroup(args: { triggers: Focusable[] }): Reactive<Focusable | undefined> {
		if (args.triggers.length < 2) {
			throw new Error('Activity zone requires two or more triggers.');
		}
		// First trigger is the default value.
		let current = $state(args.triggers[0]!);
		$effect(() => {
			// Reacts to changes in `this._current`.
			let area = this.elements.find((a) => a.key === this._current);
			if (!area) return;

			while (area && !args.triggers.includes(area.key)) {
				area = this.elements.find((a) => a.key === area?.parentId);
			}
			if (area) {
				current = area?.key;
			}
		});
		return reactive(() => current);
	}
}
