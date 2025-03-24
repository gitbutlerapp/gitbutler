import { on } from 'svelte/events';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export type FocusArea = string | null;

export type FocusableElement = {
	id: string;
	parentId: string | null;
	element: HTMLElement;
	children: string[];
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
export class FocusManager implements Reactive<string | undefined> {
	/** Elements registered using `focusable.ts` svelte action. */
	private elements: FocusableElement[] = [];

	/** Element to focusable lookup. */
	private lookup = new Map<HTMLElement, FocusableElement>();

	/** The id of the most recently focused item. */
	_current: string | undefined = $state();

	constructor() {
		$effect(() => {
			const handle = this.handleClick.bind(this);
			// Capture phase on document means this pretty much happens
			// first on any click.
			return on(document, 'mousedown', handle, { capture: true });
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
					item.element.focus();
					break;
				}
				pointer = pointer.parentElement;
			}
		}
	}

	register(id: string, parentId: string | null, element: HTMLElement) {
		let item = this.elements.find((area) => area.id === id);
		if (item) {
			this.lookup.delete(element);
			item.element = element;
			item.parentId = parentId;
		} else {
			item = { id, parentId, element, children: [] };
			this.elements.push(item);
		}

		this.lookup.set(element, item);

		const parent = this.elements.find((a) => a.id === parentId);
		if (parent) parent.children.push(id);

		item.children.push(...this.elements.filter((a) => id === a.parentId).map((a) => a.id));
	}

	unregister(id: string) {
		const index = this.elements.findIndex((area) => area.id === id);
		if (index !== -1) {
			const area = this.elements[index]!;
			this.lookup.delete(area.element);

			// Remove from parent's children
			const parent = this.elements.find((a) => a.id === area.parentId);
			if (parent) {
				parent.children = parent.children.filter((childId) => childId !== id);
			}

			this.elements.splice(index, 1);
		}

		if (this._current === id) {
			this._current = undefined;
		}
	}

	setActive(id: string) {
		if (this.elements.some((area) => area.id === id)) {
			this._current = id;
		}
	}

	focusSibling(forward = true) {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.id === currentId);
		if (!area || !area.parentId) return;

		const parent = this.elements.find((a) => a.id === area.parentId);
		if (!parent) return;

		const siblings = parent.children;
		const index = siblings.indexOf(currentId);
		const nextIndex = (index + (forward ? 1 : siblings.length - 1)) % siblings.length;

		this.setActive(siblings[nextIndex]!);
		this.elements.find((a) => a.id === siblings[nextIndex])?.element.focus();
	}

	focusParent() {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.id === currentId);
		if (area?.parentId) {
			this.setActive(area.parentId);
			this.elements.find((a) => a.id === area.parentId)?.element.focus();
		}
	}

	focusFirstChild() {
		const currentId = this._current;
		if (!currentId) return;

		const area = this.elements.find((a) => a.id === currentId);
		if (area && area.children.length > 0) {
			const firstChild = area.children[0];
			this.setActive(firstChild!);
			this.elements.find((a) => a.id === firstChild)?.element.focus();
		}
	}
}
