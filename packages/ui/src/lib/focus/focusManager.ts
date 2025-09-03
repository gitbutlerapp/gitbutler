import { focusNextTabIndex } from '@gitbutler/ui/focus/tabbable';
import {
	addAndSortByDomOrder,
	isContentEditable,
	moveElementBetweenArrays,
	removeFromArray,
	scrollIntoViewIfNeeded
} from '@gitbutler/ui/focus/utils';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { InjectionToken } from '@gitbutler/core/context';
import { on } from 'svelte/events';
import { get, writable } from 'svelte/store';

export const FOCUS_MANAGER: InjectionToken<FocusManager> = new InjectionToken('FocusManager');

const MAX_HISTORY_SIZE = 10;

export enum DefinedFocusable {
	Commit = 'commit',
	CommitList = 'commit-list',
	FileItem = 'file-item',
	FileList = 'file-list'
}

type NavigationAction = 'tab' | 'prev' | 'next' | 'exit' | 'enter';

export type KeyboardHandler = (event: KeyboardEvent) => boolean | void;

export interface FocusableOptions {
	// Identifier for this focusable element, used by custom navigation code
	id?: DefinedFocusable;
	// Custom tab order within siblings (overrides default DOM order)
	tabIndex?: number;
	// Keep focusable inactive and outside navigation hierarchy
	disabled?: boolean;
	// Treat children as a list (changes arrow key behavior)
	list?: boolean;
	// Prevent keyboard navigation from leaving this element
	trap?: boolean;
	// Automatically focus this element when registered
	activate?: boolean;
	// Don't establish parent-child relationships with other focusables
	isolate?: boolean;

	// Custom keyboard event handler, can prevent default navigation
	onKeydown?: KeyboardHandler;
	// Called when this element becomes the active focus
	onFocus?: () => void;
	// Called when this element loses focus to another element
	onBlur?: () => void;
}

interface FocusableData {
	logicalId?: DefinedFocusable;
	parentElement?: HTMLElement;
	children: HTMLElement[]; // Preserve registration order
	// Extended options
	options: FocusableOptions;
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

		// Limit to MAX_HISTORY_SIZE items
		if (this._previousElements.length > MAX_HISTORY_SIZE) {
			this._previousElements.length = MAX_HISTORY_SIZE;
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

	private getKeydownHandlers(element: HTMLElement): KeyboardHandler[] {
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

	register(options: FocusableOptions, element: HTMLElement) {
		if (!element || !element.isConnected) {
			console.warn('Attempted to register invalid or disconnected element');
			return;
		}

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

		this.establishParentChildRelationships(element, metadata, parentMeta);
		this.metadata.set(element, metadata);
		this.resolvePendingRelationships();
		this.handlePendingRegistration(element, parentMeta, options);

		if (options.activate) {
			this.setActive(element);
		}

		// Trigger onFocus if this becomes the current element
		if (options.onFocus && this._currentElement === element) {
			try {
				options.onFocus();
			} catch (error) {
				console.warn('Error in onFocus', error);
			}
		}
	}

	private establishParentChildRelationships(
		element: HTMLElement,
		metadata: FocusableData,
		parentMeta?: FocusableData
	) {
		if (!parentMeta) return;

		const parentChildren = parentMeta.children;
		const myChildren = parentChildren.filter((c) => element.contains(c));

		for (const child of myChildren) {
			this.moveChildToNewParent(child, parentChildren, metadata, element);
		}

		addAndSortByDomOrder(parentChildren, element);
	}

	private moveChildToNewParent(
		child: HTMLElement,
		parentChildren: HTMLElement[],
		newParentMetadata: FocusableData,
		newParentElement: HTMLElement
	) {
		moveElementBetweenArrays(parentChildren, newParentMetadata.children, child);

		const childMeta = this.getMetadataOrThrow(child);
		childMeta.parentElement = newParentElement;
	}

	private resolvePendingRelationships() {
		for (const pending of this.pendingRelationships) {
			const parent = this.findParent(pending);
			if (parent) {
				const pendingMeta = this.getMetadataOrThrow(pending);
				pendingMeta.parentElement = parent;
				const parentMeta = this.getMetadataOrThrow(parent);
				addAndSortByDomOrder(parentMeta.children, pending);
			}
		}
		this.pendingRelationships = this.pendingRelationships.filter(
			(p) => !this.getMetadataOrThrow(p).parentElement
		);
	}

	private handlePendingRegistration(
		element: HTMLElement,
		parentMeta: FocusableData | undefined,
		options: FocusableOptions
	) {
		if (!parentMeta && !options.isolate) {
			this.pendingRelationships.push(element);
		}
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
		if (!element || !element.isConnected) return undefined;

		let current = element.parentElement;
		let depth = 0;
		const MAX_DEPTH = 100; // Prevent infinite loops

		while (current && depth < MAX_DEPTH) {
			const focusable = this.getMetadata(current);
			if (focusable) {
				return current;
			}
			current = current.parentElement;
			depth++;
		}

		return undefined;
	}

	unregister(element: HTMLElement) {
		this.unregisterElement(element);

		// Remove pending relationships
		this.pendingRelationships = this.pendingRelationships.filter((p) => p !== element);

		// Remove from history array
		removeFromArray(this._previousElements, element);

		// Clear current if it matches
		if (this._currentElement === element) {
			const nextElement = this.getNextValidPreviousElement();
			if (nextElement) {
				this._currentElement = nextElement;
				// Remove the selected element from history since it's now current
				removeFromArray(this._previousElements, nextElement);
			} else {
				this._currentElement = undefined;
			}
			this.cursor.set(this._currentElement);
		}
	}

	setActive(element: HTMLElement) {
		if (!element || !element.isConnected || !this.isElementRegistered(element)) {
			return;
		}

		const previousElement = this._currentElement;
		const previousMeta = previousElement ? this.getMetadata(previousElement) : null;
		const newMeta = this.getMetadataOrThrow(element);

		try {
			previousMeta?.options.onBlur?.();
		} catch (error) {
			console.warn('Error in onBlur callback:', error);
		}

		// Add current element to history before changing
		if (this._currentElement) {
			this.addToPreviousElements(this._currentElement);
		}
		this._currentElement = element;

		try {
			newMeta.options.onFocus?.();
		} catch (error) {
			console.warn('Error in onFocus:', error);
		}

		this.cursor.set(element);
	}

	/**
	 * Recursively searches for the next focusable element of a specific type
	 * by traversing up the hierarchy and checking sibling branches
	 *
	 * @param searchElement - Current element to search from
	 * @param searchType - Type of focusable element to find
	 * @param forward - Whether to search forward or backward through siblings
	 * @returns The next matching element or undefined if none found
	 */
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

	/**
	 * Recursively searches within an element's children for a specific focusable type
	 *
	 * @param element - Parent element to search within
	 * @param searchType - Type of focusable element to find
	 * @returns The first matching child element or undefined if none found
	 */
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

	/**
	 * Focuses a "cousin" element - an element of the same type in a different branch
	 * of the focus tree. Used for navigating between similar elements across different
	 * sections (e.g., from one commit list to another commit list).
	 *
	 * @param forward - Whether to search forward or backward in the tree
	 * @returns True if a cousin was found and focused, false otherwise
	 */
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
		if (!element || !element.isConnected) return;

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
		if (this.tryCustomHandlers(event)) {
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

	private tryCustomHandlers(event: KeyboardEvent): boolean {
		if (!this._currentElement) return false;

		const keyHandlers = this.getKeydownHandlers(this._currentElement);

		for (const keyHandler of keyHandlers) {
			try {
				const handled = keyHandler(event);
				if (handled === true) {
					event.preventDefault();
					event.stopPropagation();
					return true;
				}
			} catch (error) {
				console.warn('Error in key handler', error);
			}
		}
		return false;
	}

	private handleBuiltinNavigation(event: KeyboardEvent, metadata: FocusableData): boolean {
		const navigationContext = this.buildNavigationContext(event, metadata);
		if (!navigationContext.action) return false;

		if (this.shouldIgnoreNavigationForInput(navigationContext)) {
			return false;
		}

		event.preventDefault();
		event.stopPropagation();

		if (this.shouldShowOutlineOnly(navigationContext)) {
			this.outline.set(true);
			return false;
		}

		return this.executeNavigationAction(navigationContext.action, {
			metaKey: event.metaKey,
			forward: !event.shiftKey,
			trap: metadata.options.trap,
			list: navigationContext.isList
		});
	}

	private buildNavigationContext(event: KeyboardEvent, metadata: FocusableData) {
		const parentElement = metadata?.parentElement;
		const parentData = parentElement ? this.getMetadataOrThrow(parentElement) : undefined;
		const isList = parentData?.options.list ?? false;
		const action = this.getNavigationAction(event.key, isList);

		return {
			action,
			isList,
			isInput: this.isInputElement(event.target),
			hasOutline: get(this.outline)
		};
	}

	private isInputElement(target: EventTarget | null): boolean {
		return (
			(target instanceof HTMLElement && isContentEditable(target)) ||
			target instanceof HTMLTextAreaElement ||
			target instanceof HTMLInputElement ||
			!this._currentElement
		);
	}

	private shouldIgnoreNavigationForInput(context: {
		action: NavigationAction | null;
		isInput: boolean;
	}): boolean {
		return context.isInput && context.action !== 'tab';
	}

	private shouldShowOutlineOnly(context: {
		action: NavigationAction | null;
		hasOutline: boolean;
	}): boolean {
		return !context.hasOutline && context.action !== 'tab';
	}

	/**
	 * Maps keyboard keys to navigation actions based on context
	 *
	 * In list contexts:
	 * - Left/Up: Navigate to previous item or exit list
	 * - Right/Down: Navigate to next item or enter child
	 *
	 * In non-list contexts:
	 * - Left/Up: Navigate to parent or previous sibling
	 * - Right/Down: Navigate to child or next sibling
	 *
	 * @param key - The keyboard key that was pressed
	 * @param isList - Whether the current element is in a list context
	 * @returns The navigation action to perform, or null if key is not handled
	 */
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

	/**
	 * Executes a navigation action with the given options
	 *
	 * Navigation behaviors:
	 * - tab: Uses native tab navigation within current element
	 * - prev/next: Navigate to sibling elements, or parent/child if no siblings
	 * - exit: Navigate to parent (or cousin with metaKey in lists)
	 * - enter: Navigate to first child (or cousin with metaKey in lists)
	 *
	 * @param action - The navigation action to perform
	 * @param options - Navigation context options
	 * @returns True if outline should be shown, false otherwise
	 */
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

	getOptions(element: HTMLElement): FocusableOptions | null {
		return element ? this.getMetadata(element)?.options || null : null;
	}

	updateElementOptions(element: HTMLElement, updates: Partial<FocusableOptions>): boolean {
		const metadata = this.getMetadata(element);
		if (!metadata) return false;
		metadata.options = { ...metadata.options, ...updates };
		return true;
	}
}
