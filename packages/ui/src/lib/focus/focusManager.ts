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
const MAX_PARENT_SEARCH_DEPTH = 100;

export enum DefinedFocusable {
	Assignments = 'assignments',
	Branch = 'branch',
	Commit = 'commit',
	CommitList = 'commit-list',
	FileItem = 'file-item',
	FileList = 'file-list'
}

export const DEFAULT_LINKS = [
	DefinedFocusable.FileItem,
	DefinedFocusable.Commit,
	DefinedFocusable.Branch,
	DefinedFocusable.Assignments
];

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
	// Link to other focusable types for boundary navigation
	linkToIds?: DefinedFocusable[];

	// Custom keyboard event handler, can prevent default navigation
	onKeydown?: KeyboardHandler;
	// Called when this element becomes the active focus or loses it
	onActive?: (value: boolean) => void;
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
			this._previousElements.splice(MAX_HISTORY_SIZE);
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

	/**
	 * Sets up global event listeners for mouse clicks and keyboard events.
	 * Must be called to enable focus management functionality.
	 */
	listen() {
		return mergeUnlisten(
			on(document, 'click', this.handleMouse, { capture: true }),
			on(document, 'keydown', this.handleKeys)
		);
	}

	private getMetadata(element: HTMLElement): FocusableData | undefined {
		return this.metadata.get(element);
	}

	private normalizeSearchTypes(
		searchTypes: DefinedFocusable | DefinedFocusable[]
	): DefinedFocusable[] {
		return Array.isArray(searchTypes) ? searchTypes : [searchTypes];
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

	/**
	 * Registers an HTML element as focusable within the focus management system.
	 * Establishes parent-child relationships, handles deferred linking, and
	 * optionally activates the element immediately upon registration.
	 */
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
		if (options.onActive && this._currentElement === element) {
			try {
				options.onActive(true);
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
		// Prevent infinite loops
		while (current && depth < MAX_PARENT_SEARCH_DEPTH) {
			const focusable = this.getMetadata(current);
			if (focusable) {
				return current;
			}
			current = current.parentElement;
			depth++;
		}

		return undefined;
	}

	/**
	 * Removes an element from the focus management system. Cleans up parent-child
	 * relationships, removes from history, and handles focus transfer if the
	 * unregistered element was currently active.
	 */
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

	/**
	 * Sets the specified element as the currently active (focused) element.
	 * Triggers onBlur callback for the previously active element and onFocus
	 * callback for the newly active element. Updates focus history.
	 */
	setActive(element: HTMLElement) {
		if (!element || !element.isConnected || !this.isElementRegistered(element)) {
			return;
		}

		const previousElement = this._currentElement;
		const previousMeta = previousElement ? this.getMetadata(previousElement) : null;
		const newMeta = this.getMetadataOrThrow(element);

		try {
			previousMeta?.options.onActive?.(false);
		} catch (error) {
			console.warn('Error in onBlur callback:', error);
		}

		// Add current element to history before changing
		if (this._currentElement) {
			this.addToPreviousElements(this._currentElement);
		}
		this._currentElement = element;

		try {
			newMeta.options.onActive?.(true);
		} catch (error) {
			console.warn('Error in onFocus:', error);
		}

		this.cursor.set(element);
	}

	/**
	 * Recursively searches for the next focusable element of specific types
	 * by traversing up the hierarchy and checking sibling branches.
	 * Finds the nearest element of any of the specified types.
	 *
	 * @param searchElement - Current element to search from
	 * @param searchTypes - Types of focusable elements to find (can be single type or array)
	 * @param forward - Whether to search forward or backward through siblings
	 * @returns The next matching element or undefined if none found
	 */
	private findNext(
		searchElement: HTMLElement,
		searchTypes: DefinedFocusable | DefinedFocusable[],
		forward: boolean
	): HTMLElement | undefined {
		const currentMeta = this.getMetadata(searchElement);
		const parentMeta = currentMeta?.parentElement && this.getMetadata(currentMeta.parentElement);
		if (!parentMeta || !this._currentElement) return;

		const typesToSearch = this.normalizeSearchTypes(searchTypes);

		const excludeIndex = parentMeta.children.indexOf(searchElement);
		const nextChildren = forward
			? parentMeta.children.slice(excludeIndex + 1)
			: parentMeta.children.slice(0, excludeIndex);

		// When searching backward, iterate in reverse order to find the closest previous element
		const childrenToSearch = forward ? nextChildren : nextChildren.reverse();

		for (const nextChild of childrenToSearch) {
			const result = this.findWithin(nextChild, typesToSearch, forward);
			if (result) return result;
		}

		return this.findNext(currentMeta.parentElement!, searchTypes, forward);
	}

	/**
	 * Recursively searches within an element's children for specific focusable types
	 *
	 * @param element - Parent element to search within
	 * @param searchTypes - Types of focusable elements to find (can be single type or array)
	 * @param forward - Whether to search forward (first match) or backward (last match)
	 * @returns The matching child element or undefined if none found
	 */
	private findWithin(
		element: HTMLElement,
		searchTypes: DefinedFocusable | DefinedFocusable[],
		forward: boolean = true
	): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata) return;

		const typesToSearch = this.normalizeSearchTypes(searchTypes);

		const children = forward ? metadata.children : metadata.children.slice().reverse();

		for (const child of children) {
			const childMeta = this.getMetadata(child);
			if (childMeta?.logicalId && typesToSearch.includes(childMeta.logicalId)) {
				return child;
			}
			const subchild = this.findWithin(child, searchTypes, forward);
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

	/**
	 * Navigates to the next or previous sibling element. If no sibling exists
	 * and we're at a list boundary with linked types configured, attempts
	 * boundary navigation to linked element types.
	 *
	 * @param forward - Whether to navigate to next (true) or previous (false) sibling
	 * @returns True if navigation succeeded, false otherwise
	 */
	focusSibling(options: { forward: boolean; metaKey: boolean }): boolean {
		const { forward, metaKey: meta } = options;
		const currentMetadata = this.getCurrentMetadata();
		const parentMetadata =
			currentMetadata?.parentElement && this.getMetadata(currentMetadata.parentElement);
		if (!parentMetadata || !this._currentElement) return false;

		const siblings = parentMetadata.children;
		const currentIndex = siblings.indexOf(this._currentElement);
		const isListWithLinks = parentMetadata.options.list && currentMetadata?.options.linkToIds;

		if (isListWithLinks && meta) {
			return this.focusLinked(forward);
		}

		// Early validation
		if (currentIndex === -1) return false;

		const targetIndex = forward ? currentIndex + 1 : currentIndex - 1;
		const hasValidSibling = targetIndex >= 0 && targetIndex < siblings.length;

		// Navigate to sibling if available
		if (hasValidSibling) {
			return this.activateAndFocus(siblings[targetIndex]);
		}
		const isAtBoundary =
			(forward && currentIndex === siblings.length - 1) || (!forward && currentIndex === 0);

		if (isListWithLinks && isAtBoundary) {
			return this.focusLinked(forward);
		}

		return false;
	}

	/**
	 * Focuses a "cousin" element - an element of the same type in a different branch
	 * of the focus tree. Used for navigating between similar elements across different
	 * sections (e.g., from one commit list to another commit list).
	 *
	 * @param forward - Whether to search forward or backward in the tree
	 * @returns True if a cousin was found and focused, false otherwise
	 */
	focusCousin(forward: boolean): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata?.parentElement || !this._currentElement) return false;

		const cousinTarget = this.findNext(
			this._currentElement,
			metadata.logicalId as DefinedFocusable,
			forward
		);
		return this.activateAndFocus(cousinTarget);
	}

	/**
	 * Focuses a linked element when navigating past list boundaries.
	 * Used for navigating to the next/previous item of a different type when
	 * reaching the end/beginning of a list. Finds the nearest element of any
	 * of the specified target types.
	 *
	 * @param forward - Whether to search forward or backward in the tree
	 * @returns True if a linked element was found and focused, false otherwise
	 */
	focusLinked(forward: boolean): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata?.parentElement || !this._currentElement) return false;

		const linkToIds = metadata.options.linkToIds;
		if (!linkToIds || linkToIds.length === 0) return false;

		// Find the nearest element of any of the target types
		const linkedTarget = this.findNext(this._currentElement, linkToIds, forward);
		return this.activateAndFocus(linkedTarget);
	}

	focusElement(element: HTMLElement): void {
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

	focusParent(): void {
		const element = this.getCurrentMetadata()?.parentElement;
		if (element) {
			this.activateAndFocus(element);
		}
	}

	/**
	 * Navigates focus to the first child element of the currently focused element.
	 */
	focusFirstChild(): boolean {
		const metadata = this.getCurrentMetadata();
		const firstChild = metadata?.children.at(0);
		return firstChild ? this.activateAndFocus(firstChild) : false;
	}

	/**
	 * Handles mouse click events to update focus. Traverses up the DOM tree
	 * from the clicked element to find the nearest registered focusable element.
	 */
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

	/**
	 * Handles keyboard events for focus navigation. Processes both custom
	 * key handlers and built-in navigation commands like arrow keys and Tab.
	 */
	handleKeydown(event: KeyboardEvent) {
		const metadata = this.getCurrentMetadata();
		if (!metadata) return;

		// Try custom handlers first
		if (this.tryCustomHandlers(event)) {
			return;
		}

		// Handle built-in navigation
		if (this.processStandardNavigation(event, metadata)) {
			this.outline.set(true);
		}
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

	/**
	 * Processes standard keyboard navigation commands (arrow keys, Tab) that are
	 * built into the focus system. Handles input validation, outline visibility,
	 * and delegates to the appropriate navigation action.
	 */
	private processStandardNavigation(event: KeyboardEvent, metadata: FocusableData): boolean {
		const navigationContext = this.buildNavigationContext(event, metadata);
		if (!navigationContext.action) return false;

		if (this.shouldIgnoreNavigationForInput(navigationContext)) {
			return false;
		}

		if (navigationContext.hasSelection) {
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
			hasSelection: this.hasTextSelection(),
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

	private hasTextSelection(): boolean {
		const selection = window.getSelection();
		return !!selection && selection.rangeCount > 0 && !selection.isCollapsed;
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
					this.focusSibling({ forward: false, metaKey });
				} else if (!this.focusSibling({ forward: false, metaKey })) {
					this.focusParent();
				}
				break;

			case 'next':
				if (trap) return true;
				if (metaKey && !isList) {
					this.focusCousin(true);
				} else if (!this.focusSibling({ forward: true, metaKey })) {
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
					this.focusSibling({ forward: true, metaKey });
				}
				break;
		}
		return true;
	}

	getOptions(element: HTMLElement): FocusableOptions | null {
		return element ? this.getMetadata(element)?.options || null : null;
	}

	/**
	 * Updates the focus options for an already registered element.
	 * Useful for dynamically changing focus behavior without re-registration.
	 */
	updateElementOptions(element: HTMLElement, updates: Partial<FocusableOptions>): boolean {
		const metadata = this.getMetadata(element);
		if (!metadata) return false;
		metadata.options = { ...metadata.options, ...updates };
		return true;
	}

	/**
	 * Pretty prints the focus tree structure to console.
	 * Shows the hierarchical relationships between focusable elements.
	 */
	debugPrintTree(logElements: boolean): void {
		const rootElements: HTMLElement[] = [];
		for (const [element, metadata] of this.metadata) {
			if (!metadata.parentElement) {
				rootElements.push(element);
			}
		}

		if (rootElements.length === 0) {
			return;
		}

		for (const root of rootElements) {
			this.printTreeNode(root, logElements, 0);
		}
	}

	private printTreeNode(element: HTMLElement, logElements: boolean, depth: number): void {
		const metadata = this.getMetadata(element);
		if (!metadata) return;

		// Build indentation
		const indent = '  '.repeat(depth);

		// Build node description
		const description = this.getElementDescription(element);
		const isCurrent = this._currentElement === element;
		const marker = isCurrent ? ' ◀── CURRENT' : '';

		// Build status indicators
		const flags: string[] = [];
		if (metadata.logicalId) flags.push(`id:${metadata.logicalId}`);
		if (metadata.options.disabled) flags.push('disabled');
		if (metadata.options.trap) flags.push('trap');
		if (metadata.options.list) flags.push('list');
		if (metadata.options.isolate) flags.push('isolate');
		const flagsStr = flags.length > 0 ? ` [${flags.join(', ')}]` : '';

		// Print current node with the actual element for hovering
		const log = `${indent}${description}${flagsStr}${marker}`;
		if (logElements) {
			// eslint-disable-next-line no-console
			console.log(log, element);
		} else {
			// eslint-disable-next-line no-console
			console.log(log);
		}

		// Print children
		const children = metadata.children;
		for (const child of children) {
			this.printTreeNode(child, logElements, depth + 1);
		}
	}

	private getElementDescription(element: HTMLElement | undefined): string {
		if (!element) return '(none)';

		const tag = element.tagName.toLowerCase();
		const classes = element.className
			? `.${element.className
					.split(' ')
					.filter((c) => c)
					.join('.')}`
			: '';
		const htmlId = element.id ? `#${element.id}` : '';

		// Truncate long class names
		const maxClassLength = 50;
		const displayClasses =
			classes.length > maxClassLength ? classes.substring(0, maxClassLength) + '...' : classes;

		return `${tag}${htmlId}${displayClasses}`.trim() || tag;
	}
}
