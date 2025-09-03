import { focusNextTabIndex } from '@gitbutler/ui/focus/tabbable';
import {
	addToArray,
	isContentEditable,
	removeFromArray,
	scrollIntoViewIfNeeded,
	sortByDomOrder
} from '@gitbutler/ui/focus/utils';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { InjectionToken } from '@gitbutler/core/context';
import { on } from 'svelte/events';
import { get, writable } from 'svelte/store';

export const FOCUS_MANAGER: InjectionToken<FocusManager> = new InjectionToken('FocusManager');

export enum DefinedFocusable {
	Commit = 'commit',
	CommitList = 'commit-list',
	FileItem = 'file-item',
	FileList = 'file-list'
}

export type Payload = Record<string, unknown>;

type NavigationAction = 'tab' | 'prev' | 'next' | 'exit' | 'enter';

export interface FocusContext<T extends Payload> {
	element: HTMLElement;
	logicalId?: DefinedFocusable;
	payload: T;
	manager: FocusManager;
}

export type KeyboardHandler<T extends Payload> = (
	event: KeyboardEvent,
	context: FocusContext<T>
) => boolean | void;

export interface FocusableOptions<T extends Payload = Payload> {
	// Identifier for this focusable element, used by custom navigation code
	id?: DefinedFocusable;
	// Custom tab order within siblings (overrides default DOM order)
	tabIndex?: number;
	// Keep focusable inactive and outside navigation hierarchy
	disabled?: boolean;
	// Custom data attached to this focusable element, passed to event handlers
	payload?: T;
	// Treat children as a list (changes arrow key behavior)
	list?: boolean;
	// Prevent keyboard navigation from leaving this element
	trap?: boolean;
	// Automatically focus this element when registered
	activate?: boolean;
	// Don't establish parent-child relationships with other focusables
	isolate?: boolean;

	// Custom keyboard event handler, can prevent default navigation
	onKeydown?: KeyboardHandler<T>;
	// Called when this element becomes the active focus
	onFocus?: (context: FocusContext<T>) => void;
	// Called when this element loses focus to another element
	onBlur?: (context: FocusContext<T>) => void;
}

interface FocusableData<T extends Payload = any> {
	logicalId?: DefinedFocusable;
	parentElement?: HTMLElement;
	children: HTMLElement[]; // Preserve registration order
	// Extended options
	options: FocusableOptions<T>;
}

/**
 * Robust FocusManager that handles out-of-order registration using:
 * 1. Deferred linking for parent-child relationships
 * 2. DOM traversal as fallback for parent discovery
 * 3. Lazy resolution during navigation operations
 */
export class FocusManager {
	// Physical registry: element -> metadata
	private metadata = new Map<HTMLElement, FocusableData>();

	// Deferred relationships: parent hasn't been registered yet
	private pendingRelationships: HTMLElement[] = [];

	// Current focused element (physical)
	private _currentElement: HTMLElement | undefined;

	// Previously focused elements (most recent first, limited to 10 items).
	private _previousElements: HTMLElement[] = [];

	readonly cursor = writable<HTMLElement | undefined>();
	readonly outline = writable(false);

	private handleMouse = this.handleClick.bind(this);
	private handleKeys = this.handleKeydown.bind(this);

	constructor() {}

	private addToPreviousElements(element: HTMLElement) {
		// Remove element if it already exists in the array
		const existingIndex = this._previousElements.indexOf(element);
		if (existingIndex !== -1) {
			this._previousElements.splice(existingIndex, 1);
		}

		// Add to front
		this._previousElements.unshift(element);

		// Limit to 10 items
		if (this._previousElements.length > 10) {
			this._previousElements.length = 10;
		}
	}

	private getNextValidPreviousElement(): HTMLElement | undefined {
		// Find the first connected element in the history
		for (let i = 0; i < this._previousElements.length; i++) {
			const element = this._previousElements[i];
			if (element.isConnected) {
				return element;
			}
		}

		// Clean up disconnected elements
		this._previousElements = this._previousElements.filter((el) => el.isConnected);
		return undefined;
	}

	listen() {
		return mergeUnlisten(
			on(document, 'click', this.handleMouse, { capture: true }),
			on(document, 'keydown', this.handleKeys)
		);
	}

	private getMetadata(element: HTMLElement): FocusableData | undefined {
		return this.metadata.get(element);
	}

	private getKeydownHandlers(element: HTMLElement): KeyboardHandler<any>[] {
		let metadata = this.metadata.get(element);
		const handlers = [];

		while (metadata) {
			if (metadata.options.onKeydown) {
				handlers.push(metadata.options.onKeydown);
			}
			const parentElement = metadata?.parentElement;
			metadata = parentElement ? this.getMetadata(parentElement) : undefined;
		}
		return handlers;
	}

	private getMetadataOrThrow(element: HTMLElement): FocusableData {
		const metadata = this.getMetadata(element);
		if (!metadata) {
			throw new Error(`Element not registered in focus manager`);
		}
		return metadata;
	}

	private isElementRegistered(element: HTMLElement): boolean {
		return this.metadata.has(element);
	}

	private getCurrentMetadata(): FocusableData | undefined {
		return this._currentElement ? this.getMetadata(this._currentElement) : undefined;
	}

	register<T extends Payload>(options: FocusableOptions<T>, element: HTMLElement) {
		const { id: logicalId } = options;
		this.unregisterElement(element);

		const parentElement = options.isolate ? undefined : this.findParent(element);
		const parentMeta = parentElement ? this.getMetadata(parentElement) : undefined;

		const metadata: FocusableData = {
			logicalId,
			parentElement,
			children: [],
			options
		};

		if (parentMeta) {
			const parentChildren = parentMeta.children;
			const myChildren = parentChildren.filter((c) => element.contains(c));
			for (const child of myChildren) {
				parentChildren.splice(parentChildren.indexOf(child), 1);

				metadata.children.push(child);
				sortByDomOrder(metadata.children);
				const childMeta = this.getMetadataOrThrow(child);
				childMeta.parentElement = element;
			}
			addToArray(parentChildren, element);
			sortByDomOrder(parentMeta.children);
		}

		this.metadata.set(element, metadata);

		for (const pending of this.pendingRelationships) {
			const parent = this.findParent(pending);
			if (parent) {
				const pendingMeta = this.getMetadataOrThrow(pending);
				pendingMeta.parentElement = parent;
				const parentMeta = this.getMetadataOrThrow(parent);
				parentMeta.children.push(pending);
				sortByDomOrder(parentMeta.children);
			}
		}
		this.pendingRelationships = this.pendingRelationships.filter(
			(p) => !this.getMetadataOrThrow(p).parentElement
		);

		if (!parentMeta && !options.isolate) {
			this.pendingRelationships.push(element);
		}

		if (options.activate) {
			this.setActive(element);
		}

		// Trigger onFocus if this becomes the current element
		if (options.onFocus && this._currentElement === element) {
			options.onFocus(this.createContext(element, metadata));
		}
	}

	private createContext<T extends Payload>(
		element: HTMLElement,
		metadata: FocusableData<T>
	): FocusContext<T> {
		return {
			element,
			logicalId: metadata.logicalId,
			payload: metadata.options.payload as T,
			manager: this
		};
	}

	private unregisterElement(element: HTMLElement) {
		const meta = this.metadata.get(element);
		if (!meta) return;

		const parentElement = meta.parentElement;
		const parentMeta = parentElement ? this.metadata.get(parentElement) : undefined;

		if (meta.parentElement && parentMeta) {
			removeFromArray(parentMeta.children, element);
		}

		for (const child of meta.children) {
			const childMeta = this.getMetadataOrThrow(child);
			childMeta.parentElement = parentElement;
			parentMeta?.children.push(child);
		}

		this.metadata.delete(element);
	}

	private findParent(element: HTMLElement): HTMLElement | undefined {
		let current = element.parentElement;
		while (current) {
			const focusable = this.getMetadata(current);
			if (focusable) {
				return current;
			}
			current = current.parentElement;
		}
	}

	unregister(element: HTMLElement) {
		if (element) {
			this.unregisterElement(element);
		}

		// Remove pending relationships
		this.pendingRelationships = this.pendingRelationships.filter((p) => p !== element);

		// Remove from history array
		const historyIndex = this._previousElements.indexOf(element);
		if (historyIndex !== -1) {
			this._previousElements.splice(historyIndex, 1);
		}

		// Clear current if it matches
		if (this._currentElement === element) {
			const nextElement = this.getNextValidPreviousElement();
			if (nextElement) {
				this._currentElement = nextElement;
				// Remove the selected element from history since it's now current
				const index = this._previousElements.indexOf(nextElement);
				if (index !== -1) {
					this._previousElements.splice(index, 1);
				}
			} else {
				this._currentElement = undefined;
			}
			this.cursor.set(this._currentElement);
		}
	}

	setActive(element: HTMLElement) {
		if (this.isElementRegistered(element)) {
			const previousElement = this._currentElement;
			const previousMeta = previousElement ? this.getMetadata(previousElement) : null;
			const newMeta = this.getMetadataOrThrow(element);

			if (previousElement && previousMeta?.options.onBlur) {
				previousMeta.options.onBlur(this.createContext(previousElement, previousMeta));
			}

			// Add current element to history before changing
			if (this._currentElement) {
				this.addToPreviousElements(this._currentElement);
			}
			this._currentElement = element;

			if (newMeta.options.onFocus) {
				newMeta.options.onFocus(this.createContext(element, newMeta));
			}

			this.cursor.set(element);
		}
	}

	private findNext(
		searchElement: HTMLElement,
		searchType: DefinedFocusable,
		forward: boolean
	): HTMLElement | undefined {
		const currentMeta = this.getMetadata(searchElement);
		const parentMeta = currentMeta?.parentElement && this.getMetadata(currentMeta.parentElement);
		if (!parentMeta || !this._currentElement) return;

		const excludeIndex = parentMeta.children.indexOf(searchElement);
		const nextChildren = forward
			? parentMeta.children.slice(excludeIndex + 1)
			: parentMeta.children.slice(0, excludeIndex);

		for (const nextChild of nextChildren) {
			const result = this.findWithin(nextChild, searchType);
			if (result) return result;
		}

		return this.findNext(currentMeta.parentElement!, searchType, forward);
	}

	private findWithin(element: HTMLElement, searchType: DefinedFocusable): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata) return;
		for (const child of metadata.children) {
			const childMeta = this.getMetadata(child);
			if (childMeta?.logicalId === searchType) {
				return child;
			}
			const subchild = this.findWithin(child, searchType);
			if (subchild) {
				return subchild;
			}
		}
	}

	private activateAndFocus(element: HTMLElement | undefined): boolean {
		if (!element) return false;
		this.setActive(element);
		this.focusElement(element);
		return true;
	}

	focusSibling(forward = true): boolean {
		const metadata = this.getCurrentMetadata();
		const parentMeta = metadata?.parentElement && this.getMetadata(metadata.parentElement);
		if (!parentMeta) return false;

		const siblings = parentMeta.children;
		const currentIndex = siblings.indexOf(this._currentElement!);
		const nextIndex = forward ? currentIndex + 1 : currentIndex - 1;

		return currentIndex !== -1 && nextIndex >= 0 && nextIndex < siblings.length
			? this.activateAndFocus(siblings[nextIndex])
			: false;
	}

	focusCousin(forward: boolean) {
		const metadata = this.getCurrentMetadata();
		if (!metadata?.parentElement || !this._currentElement) return false;

		const cousinTarget = this.findNext(
			this._currentElement,
			metadata.logicalId as DefinedFocusable,
			forward
		);
		return this.activateAndFocus(cousinTarget);
	}

	focusElement(element: HTMLElement) {
		if (element.tabIndex !== -1) {
			element.focus();
		} else {
			if (document.activeElement && document.activeElement instanceof HTMLElement) {
				document.activeElement.blur();
			}
		}
		scrollIntoViewIfNeeded(element);
	}

	focusParent() {
		const element = this.getCurrentMetadata()?.parentElement;
		if (element) {
			this.activateAndFocus(element);
		}
	}

	focusFirstChild(): boolean {
		const metadata = this.getCurrentMetadata();
		const firstChild = metadata?.children.at(0);
		return firstChild ? this.activateAndFocus(firstChild) : false;
	}

	handleClick(e: MouseEvent) {
		// Ignore keyboard initiated clicks.
		if (e.detail === 0) {
			return;
		}
		if (e.target instanceof HTMLElement) {
			let pointer: HTMLElement | null = e.target;
			while (pointer) {
				if (this.isElementRegistered(pointer)) {
					this.setActive(pointer);
					this.outline.set(false);
					break;
				}
				pointer = pointer.parentElement;
			}
		}
	}

	handleKeydown(event: KeyboardEvent) {
		const metadata = this.getCurrentMetadata();
		if (!metadata) return;

		// Try custom handlers first
		if (this.tryCustomHandlers(event, metadata)) {
			return;
		}

		// Handle built-in navigation
		if (this.handleBuiltinNavigation(event, metadata)) {
			this.outline.set(true);
		}
	}

	/**
	 * This is a hack to make the focus manager interact better with context
	 * menus, and should be refactored. Ideally we deprecate `focusTrap` and
	 * adapt the focusable to work well with menus.
	 */
	ignoreKeys() {
		return (
			document.activeElement &&
			document.activeElement !== document.body &&
			this._currentElement &&
			!this._currentElement.contains(document.activeElement)
		);
	}

	private tryCustomHandlers(event: KeyboardEvent, metadata: FocusableData): boolean {
		const keyHandlers = this.getKeydownHandlers(this._currentElement!);
		const context = this.createContext(this._currentElement!, metadata);

		for (const keyHandler of keyHandlers) {
			const handled = keyHandler(event, context);
			if (handled === true) {
				event.preventDefault();
				event.stopPropagation();
				return true;
			}
		}
		return false;
	}

	private handleBuiltinNavigation(event: KeyboardEvent, metadata: FocusableData): boolean {
		const parentElement = metadata?.parentElement;
		const parentData = parentElement ? this.getMetadataOrThrow(parentElement) : undefined;
		const list = parentData?.options.list ?? false;

		const navigationAction = this.getNavigationAction(event.key, list);
		if (!navigationAction) return false;

		const isInput =
			(event.target instanceof HTMLElement && isContentEditable(event.target)) ||
			event.target instanceof HTMLTextAreaElement ||
			event.target instanceof HTMLInputElement ||
			!this._currentElement;

		if (isInput && navigationAction !== 'tab') {
			return false;
		}

		// It feels more predictable to make the outline visible on the first
		// keyboard interaction, rather than potentially moving the cursor and
		// then making it visible.
		if (!get(this.outline) && navigationAction !== 'tab') {
			this.outline.set(true);
			event.stopPropagation();
			event.preventDefault();
			return false;
		}

		event.preventDefault();
		event.stopPropagation();

		return this.executeNavigationAction(navigationAction, {
			metaKey: event.metaKey,
			forward: !event.shiftKey,
			trap: metadata.options.trap,
			list
		});
	}

	private getNavigationAction(key: string, isList: boolean): NavigationAction | null {
		const keyMap: Record<string, NavigationAction> = {
			Tab: 'tab',
			ArrowLeft: isList ? 'exit' : 'prev',
			ArrowRight: isList ? 'enter' : 'next',
			ArrowUp: isList ? 'prev' : 'exit',
			ArrowDown: isList ? 'next' : 'enter'
		};
		return keyMap[key] ?? null;
	}

	private executeNavigationAction(
		action: NavigationAction,
		options: {
			metaKey: boolean;
			list: boolean;
			forward: boolean;
			trap?: boolean;
		}
	): boolean {
		const { metaKey, list: isList, forward = true, trap } = options;
		switch (action) {
			case 'tab':
				if (this._currentElement) {
					focusNextTabIndex({ container: this._currentElement, forward });
				}
				return false; // do not toggle outline

			case 'prev':
				if (trap) return true;
				if (metaKey && !isList) {
					this.focusCousin(false);
				} else if (!this.focusSibling(false)) {
					this.focusParent();
				}
				break;

			case 'next':
				if (trap) return true;
				if (metaKey && !isList) {
					this.focusCousin(true);
				} else if (!this.focusSibling(true)) {
					this.focusFirstChild();
				}
				break;

			case 'exit':
				if (trap) return true;
				if (metaKey && isList) {
					this.focusCousin(false);
				} else {
					this.focusParent();
				}
				break;

			case 'enter':
				if (trap) return true;
				if (metaKey && isList) {
					this.focusCousin(true);
				} else if (!this.focusFirstChild()) {
					this.focusSibling(true);
				}
				break;
		}
		return true;
	}

	getOptions<T extends Payload>(element: HTMLElement): FocusableOptions<T> | null {
		return element ? this.getMetadata(element)?.options || null : null;
	}

	updateElementOptions<T extends Payload>(
		element: HTMLElement,
		updates: Partial<FocusableOptions<T>>
	): boolean {
		const metadata = this.getMetadata(element);
		if (!metadata) return false;
		metadata.options = { ...metadata.options, ...updates };
		return true;
	}
}
