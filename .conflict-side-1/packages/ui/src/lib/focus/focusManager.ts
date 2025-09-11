import { focusNextTabIndex } from '$lib/focus/tabbable';
import {
	addAndSortByDomOrder,
	isContentEditable,
	moveElementBetweenArrays,
	removeFromArray,
	scrollIntoViewIfNeeded
} from '$lib/focus/utils';
import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
import { InjectionToken } from '@gitbutler/core/context';
import { on } from 'svelte/events';
import { get, writable } from 'svelte/store';

export const FOCUS_MANAGER: InjectionToken<FocusManager> = new InjectionToken('FocusManager');

const MAX_HISTORY_SIZE = 10;
const MAX_PARENT_SEARCH_DEPTH = 100;
const MAX_DESCENDANT_DEPTH = 10;
const MAX_TRAVERSAL_DEPTH = 20;

type NavigationAction = 'tab' | 'left' | 'right' | 'up' | 'down';

export type KeyboardHandler = (event: KeyboardEvent) => boolean | void;

export type NavigationContext = {
	action: NavigationAction | null;
	trap?: boolean;
	inVertical?: boolean;
	isInput?: boolean;
	// User selected text detected.
	hasSelection?: boolean;
	hasOutline?: boolean;
};

export interface FocusableOptions {
	// Custom tab order within siblings (overrides default DOM order)
	tabIndex?: number;
	// Keep focusable inactive and outside navigation hierarchy
	disabled?: boolean;
	// Treat children as vertically oriented (changes arrow key behavior, automatically skips during navigation)
	vertical?: boolean;
	// Prevent keyboard navigation from leaving this element
	trap?: boolean;
	// Automatically focus this element when registered
	activate?: boolean;
	// Don't establish parent-child relationships with other focusables
	isolate?: boolean;
	// Never highlight the element
	dim?: boolean;
	// Automatically trigger onAction when this element becomes active
	autoAction?: boolean;

	// Custom keyboard event handler, can prevent default navigation
	onKeydown?: KeyboardHandler;
	// Called when this element becomes the active focus or loses it
	onActive?: (value: boolean) => void;
	// Called when Space or Enter is pressed on this focused element, or when autoAction is true and element becomes active
	onAction?: () => boolean | void;
}

interface FocusableData {
	parentElement?: HTMLElement;
	children: HTMLElement[]; // Preserve registration order
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
	private currentElement: HTMLElement | undefined;

	// Previously focused elements (most recent first, limited to 10 items).
	private previousElements: HTMLElement[] = [];

	// Cache for last active index of vertical focusables (element -> last active child index)
	private indexCache = new Map<HTMLElement, number>();

	readonly cursor = writable<HTMLElement | undefined>();
	readonly outline = writable(false);

	private handleMouse = this.handleClick.bind(this);
	private handleKeys = this.handleKeydown.bind(this);

	private addToPreviousElements(element: HTMLElement) {
		const existingIndex = this.previousElements.indexOf(element);
		if (existingIndex !== -1) {
			this.previousElements.splice(existingIndex, 1);
		}

		this.previousElements.unshift(element);

		if (this.previousElements.length > MAX_HISTORY_SIZE) {
			this.previousElements.splice(MAX_HISTORY_SIZE);
		}
	}

	private getValidPreviousElement(): HTMLElement | undefined {
		// Find the first connected element in the history
		for (let i = 0; i < this.previousElements.length; i++) {
			const element = this.previousElements[i];
			if (element.isConnected) {
				return element;
			}
		}

		// Clean up disconnected elements
		this.previousElements = this.previousElements.filter((el) => el.isConnected);
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
		return this.currentElement ? this.getMetadata(this.currentElement) : undefined;
	}

	/**
	 * Gets siblings of an element in a specified direction, excluding the element itself.
	 * Returns reversed order when searching backward for closest-first traversal.
	 */
	private getSiblingsInDirection(
		siblings: HTMLElement[],
		currentElement: HTMLElement,
		forward: boolean
	): HTMLElement[] {
		const currentIndex = siblings.indexOf(currentElement);
		if (currentIndex === -1) return [];

		const selectedSiblings = forward
			? siblings.slice(currentIndex + 1)
			: siblings.slice(0, currentIndex);

		// When searching backward, reverse to get closest-first order
		return forward ? selectedSiblings : selectedSiblings.reverse();
	}

	/**
	 * Gets children of an element in a specified direction.
	 */
	private getChildrenInDirection(children: HTMLElement[], forward: boolean): HTMLElement[] {
		return forward ? children : children.slice().reverse();
	}

	/**
	 * Gets the parent element's metadata for a given metadata object.
	 */
	private getParentMetadata(metadata: FocusableData | undefined): FocusableData | undefined {
		if (!metadata?.parentElement) return undefined;
		return this.getMetadata(metadata.parentElement);
	}

	/**
	 * Determines if an element should be skipped during navigation.
	 * An element is skipped if it has vertical: true or if it has children.
	 * @param metadata - The element's metadata
	 * @returns True if the element should be skipped, false otherwise
	 */
	private shouldSkipElement(metadata: FocusableData): boolean {
		return metadata.options.vertical === true || metadata.children.length > 0;
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

		this.unregisterElement(element);

		const parentElement = options.isolate ? undefined : this.findParent(element);
		const parentMeta = parentElement ? this.getMetadata(parentElement) : undefined;

		const metadata: FocusableData = {
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
	 * relationships, removes from history, clears cache entries, and handles focus
	 * transfer if the unregistered element was currently active.
	 */
	unregister(element: HTMLElement) {
		this.unregisterElement(element);

		// Remove pending relationships
		this.pendingRelationships = this.pendingRelationships.filter((p) => p !== element);

		// Remove from history array
		removeFromArray(this.previousElements, element);

		// Clean up cache entry for this element
		this.indexCache.delete(element);

		// Clear current if it matches
		if (this.currentElement === element) {
			const previousElement = this.getValidPreviousElement();
			if (previousElement) {
				this.currentElement = previousElement;
				// Remove the selected element from history since it's now current
				removeFromArray(this.previousElements, previousElement);
			} else {
				this.currentElement = undefined;
			}
			this.cursor.set(this.currentElement);
		}
	}

	/**
	 * Fires onActive callbacks for an element and all its parent focusables
	 */
	private triggerOnActiveCallbacks(element: HTMLElement, active: boolean): void {
		let metadata = this.getMetadata(element);
		while (metadata) {
			try {
				metadata.options.onActive?.(active);
				// If autoAction is enabled and element is becoming active, also trigger onAction
				if (active && metadata.options.autoAction && metadata.options.onAction) {
					metadata.options.onAction();
				}
			} catch (error) {
				console.warn(`Error in onActive(${active}) callback:`, error);
			}
			const parentElement = metadata.parentElement;
			if (!parentElement) break;
			metadata = this.getMetadata(parentElement);
		}
	}

	private findFocusableChild(element: HTMLElement): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata?.children.length) return undefined;

		// Try preferred child first, then iterate through all children
		const childrenToTry = [this.getPreferredChild(element), ...metadata.children].filter(
			(child, index, arr) => child && arr.indexOf(child) === index
		); // Remove duplicates

		for (const child of childrenToTry) {
			const activated = this.setActive(child!);
			if (activated) return activated;
		}

		return undefined;
	}

	/**
	 * Sets the specified element as the currently active (focused) element.
	 * If the element should be skipped (vertical: true or has children), it will try to focus the preferred child instead.
	 * Triggers onBlur callback for the previously active element and onFocus
	 * callback for the newly active element. Also fires onActive callbacks
	 * for all parent focusables. Updates focus history and caches the active index
	 * for vertical focusables.
	 */
	setActive(element: HTMLElement): HTMLElement | undefined {
		if (!element?.isConnected || !this.isElementRegistered(element)) return undefined;

		const metadata = this.getMetadata(element);
		if (!metadata) {
			console.warn('Could not find metadata', element);
			return undefined;
		}

		const targetElement = this.findFocusableChild(element) || element;
		const previousElement = this.currentElement;

		if (previousElement) {
			this.triggerOnActiveCallbacks(previousElement, false);
			this.addToPreviousElements(previousElement);
		}

		this.currentElement = targetElement;
		this.cacheActiveIndex(targetElement);
		this.triggerOnActiveCallbacks(targetElement, true);
		this.cursor.set(targetElement);

		return targetElement;
	}

	/**
	 * Gets the preferred child element to focus within any container.
	 * Uses cached index if available, otherwise falls back to first child.
	 *
	 * @param containerElement - The container element to get a child from
	 * @returns The preferred child element to focus, or undefined if no children
	 */
	private getPreferredChild(containerElement: HTMLElement): HTMLElement | undefined {
		const containerMetadata = this.getMetadata(containerElement);
		if (!containerMetadata?.children.length) {
			return undefined;
		}

		const cachedIndex = this.indexCache.get(containerElement);
		if (cachedIndex !== undefined && cachedIndex < containerMetadata.children.length) {
			const cachedChild = containerMetadata.children[cachedIndex];
			if (cachedChild && cachedChild.isConnected) {
				return cachedChild;
			}
		}

		return containerMetadata.children.at(0);
	}

	/**
	 * Traverses the hierarchy starting from a container, following cached positions
	 * until it finds the deepest descendant that has a cached value. This provides more
	 * contextual navigation by taking you to exactly where you were working in nested containers.
	 *
	 * @param startingContainer - The container to start traversing from
	 * @returns The deepest cached descendant element to focus
	 */
	private getDeepestCachedDescendant(startingContainer: HTMLElement): HTMLElement | undefined {
		let current = startingContainer;
		const visited = new Set<HTMLElement>();
		while (visited.size < MAX_DESCENDANT_DEPTH) {
			// Prevent infinite loops
			if (visited.has(current)) {
				break;
			}
			visited.add(current);

			const preferredChild = this.getPreferredChild(current);
			if (!preferredChild) {
				return current;
			}

			const childHasCache = this.indexCache.has(preferredChild);

			if (!childHasCache) {
				return preferredChild;
			}

			current = preferredChild;
		}

		return current;
	}

	/**
	 * Recursively searches for the next non-skipped focusable element within vertical contexts.
	 * Traverses up the hierarchy while parent elements have vertical: true, ensuring navigation
	 * stays within the same column/vertical structure.
	 *
	 * @param searchElement - Current element to search from
	 * @param forward - Whether to search forward (next) or backward (previous) through siblings
	 * @returns The next non-skipped focusable element or undefined if none found
	 */
	private findNextInVertical(
		searchElement: HTMLElement,
		forward: boolean
	): HTMLElement | undefined {
		const currentMeta = this.getMetadata(searchElement);
		const parentMeta = this.getParentMetadata(currentMeta);
		if (!currentMeta || !parentMeta || !this.currentElement) return undefined;

		// Exit if parent is not vertical to stay within the same column
		if (!parentMeta.options.vertical) {
			return undefined;
		}

		const childrenToSearch = this.getSiblingsInDirection(
			parentMeta.children,
			searchElement,
			forward
		);

		for (const nextChild of childrenToSearch) {
			const result = this.findNonSkippedDescendant(nextChild, forward);
			if (result) return result;
		}

		if (parentMeta.options.vertical && currentMeta.parentElement) {
			return this.findNextInVertical(currentMeta.parentElement, forward);
		}

		return undefined;
	}

	/**
	 * Recursively searches within an element's children for the first non-skipped focusable element.
	 * If the element has no children, returns the element itself.
	 *
	 * @param element - Parent element to search within
	 * @param forward - Whether to search forward (first match) or backward (last match) through children
	 * @returns The first non-skipped child element, the element itself if no children, or undefined if no valid element found
	 */
	private findNonSkippedDescendant(
		element: HTMLElement,
		forward: boolean = true
	): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata) return undefined;

		const children = this.getChildrenInDirection(metadata.children, forward);

		if (children.length === 0) {
			return element;
		}

		for (const child of children) {
			const childMeta = this.getMetadata(child);

			// Skip elements (those with vertical: true or children) entirely
			if (childMeta && this.shouldSkipElement(childMeta)) {
				// Search within skipped elements but don't return them directly
				const subchild = this.findNonSkippedDescendant(child, forward);
				if (subchild) {
					return subchild;
				}
				continue;
			}

			return child;
		}

		return undefined;
	}

	activateAndFocus(element: HTMLElement | undefined): boolean {
		if (!element) return false;
		const activatedElement = this.setActive(element);
		if (activatedElement) {
			this.focusElement(activatedElement);
		}
		return true;
	}

	/**
	 * Navigates to the right or previous sibling element. If no sibling exists
	 * and we're at a vertical boundary with linked types configured, attempts
	 * boundary navigation to linked element types.
	 *
	 * @param forward - Whether to navigate to right (true) or previous (false) sibling
	 * @returns True if navigation succeeded, false otherwise
	 */
	focusSibling(options: { forward: boolean; metaKey?: boolean }): boolean {
		const { forward } = options;
		const currentMetadata = this.getCurrentMetadata();
		const parentMetadata = this.getParentMetadata(currentMetadata);
		if (!parentMetadata || !this.currentElement) return false;

		const siblings = parentMetadata.children;
		const currentIndex = siblings.indexOf(this.currentElement);

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

		if (isAtBoundary) {
			return this.focusColumnSibling(forward);
		}

		return false;
	}

	/**
	 * Focuses a linked element when navigating past vertical boundaries.
	 * Finds the next non-skipped focusable, traversing parents while vertical: true.
	 *
	 * @param forward - Whether to search forward or backward in the tree
	 * @returns True if a linked element was found and focused, false if no focusable was found
	 */
	focusColumnSibling(forward: boolean): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata?.parentElement || !this.currentElement) return false;

		// Find the next non-skipped focusable, traversing parents while vertical: true
		const linkedTarget = this.findNextInVertical(this.currentElement, forward);
		if (!linkedTarget) return false;

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

	focusParent(forward: boolean = true): void {
		const currentMetadata = this.getCurrentMetadata();
		if (!currentMetadata?.parentElement || !this.currentElement) return;

		let currentElement = this.currentElement;
		let parentElement = currentMetadata.parentElement;
		let parentMetadata = this.getMetadata(parentElement);
		let depth = 0;

		while (parentElement && parentMetadata && depth < MAX_TRAVERSAL_DEPTH) {
			// If parent should be skipped, try to find focusable child in the specified direction
			if (this.shouldSkipElement(parentMetadata)) {
				// Only look for focusable descendents a skipped focusable is of vertical type,
				// otherwise keep traversing the parent elements.
				if (parentMetadata.options.vertical) {
					const siblingsToSearch = this.getSiblingsInDirection(
						parentMetadata.children,
						currentElement,
						forward
					);

					for (const sibling of siblingsToSearch) {
						const siblingMeta = this.getMetadataOrThrow(sibling);
						if (!this.shouldSkipElement(siblingMeta)) {
							// If sibling itself is non-skippable, focus it
							this.activateAndFocus(sibling);
							return;
						} else {
							// If sibling should be skipped, try to focus its preferred child
							const preferredChild = this.getPreferredChild(sibling);
							if (preferredChild) {
								this.activateAndFocus(preferredChild);
								return;
							}
						}
					}
				}

				// If we didn't find anything at this level, move up to the next parent
				if (!parentMetadata.parentElement) break;
				currentElement = parentElement;
				parentElement = parentMetadata.parentElement;
				parentMetadata = this.getMetadata(parentElement);
				if (!parentMetadata) break;
				depth++;
			} else {
				// Found a non-skip parent, focus it
				this.activateAndFocus(parentElement);
				return;
			}
		}

		// Fallback: if we've exhausted all parents and haven't found anything, do nothing
	}

	/**
	 * Navigates focus to the preferred child element of the currently focused element.
	 * Uses cached position if available, otherwise falls back to first child.
	 * If the preferred child should be skipped (vertical: true or has children), it will try to focus its preferred child instead.
	 */
	focusChild(): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata || !this.currentElement) return false;

		// Get the preferred child (cached or first child)
		const preferredChild = this.getPreferredChild(this.currentElement);
		if (!preferredChild) return false;

		const childMetadata = this.getMetadata(preferredChild);

		// If the preferred child should be skipped, try to focus its preferred child (cached or first)
		if (childMetadata && this.shouldSkipElement(childMetadata)) {
			const preferredGrandChild = this.getPreferredChild(preferredChild);
			if (preferredGrandChild) {
				return this.activateAndFocus(preferredGrandChild);
			}
			// If no grandchildren, try the next sibling
			const siblings = metadata.children;
			const currentIndex = siblings.indexOf(preferredChild);
			const nextSibling = siblings.at(currentIndex + 1);
			return nextSibling ? this.activateAndFocus(nextSibling) : false;
		}

		return this.activateAndFocus(preferredChild);
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

		const navigationContext = this.buildNavigationContext(event, metadata);

		if (this.shouldIgnoreNavigationForInput(navigationContext)) {
			return false;
		}

		if (navigationContext.hasSelection) {
			return false;
		}

		if (this.shouldShowOutlineOnly(navigationContext)) {
			this.outline.set(true);
			event.stopPropagation();
			event.preventDefault();
			return false;
		}

		// Handle Space/Enter action keys first
		if (this.tryActionHandler(event)) {
			return;
		}

		// Try custom handlers
		if (this.tryCustomHandlers(event)) {
			return;
		}

		// Handle built-in navigation
		if (this.processStandardNavigation(event, navigationContext)) {
			this.outline.set(true);
		}
	}

	private tryActionHandler(event: KeyboardEvent): boolean {
		if (!this.currentElement) return false;

		// Check if the key is Space or Enter
		if (event.key !== ' ' && event.key !== 'Enter') return false;

		const metadata = this.getMetadata(this.currentElement);
		if (!metadata?.options.onAction) return false;

		try {
			metadata.options.onAction();
			return true;
		} catch (error) {
			console.warn('Error in onAction handler:', error);
		}

		return false;
	}

	private tryCustomHandlers(event: KeyboardEvent): boolean {
		if (!this.currentElement) return false;

		const keyHandlers = this.getKeydownHandlers(this.currentElement);

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
	private processStandardNavigation(
		event: KeyboardEvent,
		navigationContext: NavigationContext
	): boolean {
		if (!navigationContext.action) return false;

		event.preventDefault();
		event.stopPropagation();

		return this.executeNavigationAction(navigationContext.action, {
			metaKey: event.metaKey,
			forward: !event.shiftKey,
			trap: navigationContext.trap,
			inVertical: navigationContext.inVertical
		});
	}

	private buildNavigationContext(event: KeyboardEvent, metadata: FocusableData): NavigationContext {
		const parentElement = metadata?.parentElement;
		const parentData = parentElement ? this.getMetadataOrThrow(parentElement) : undefined;
		const inVertical = parentData?.options.vertical ?? false;
		const action = this.getNavigationAction(event.key);

		return {
			action,
			trap: metadata.options.trap,
			inVertical,
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
			!this.currentElement
		);
	}

	private hasTextSelection(): boolean {
		const selection = window.getSelection();
		return !!selection && selection.rangeCount > 0 && !selection.isCollapsed;
	}

	private shouldIgnoreNavigationForInput(context: {
		action: NavigationAction | null;
		isInput?: boolean;
	}): boolean {
		return (context.isInput && context.action !== 'tab') || false;
	}

	private shouldShowOutlineOnly(context: {
		action: NavigationAction | null;
		hasOutline?: boolean;
	}): boolean {
		return (!context.hasOutline && context.action !== null) || false;
	}

	/**
	 * Maps keyboard keys to navigation actions.
	 */
	private getNavigationAction(key: string): NavigationAction | null {
		const keyMap: Record<string, NavigationAction> = {
			Tab: 'tab',
			ArrowLeft: 'left',
			ArrowRight: 'right',
			ArrowUp: 'up',
			ArrowDown: 'down'
		};
		return keyMap[key] ?? null;
	}

	/**
	 * Executes a navigation action with the given options
	 *
	 * Navigation behaviors:
	 * - tab: Uses native tab navigation within current element
	 * - prev/right: Navigate to sibling elements, or parent/child if no siblings
	 * - exit: Navigate to parent (or cousin with metaKey in vertical containers)
	 * - enter: Navigate to first child (or cousin with metaKey in vertical containers)
	 *
	 * @param action - The navigation action to perform
	 * @param options - Navigation context options
	 * @returns True if outline should be shown, false otherwise
	 */
	private executeNavigationAction(
		action: NavigationAction,
		options: {
			metaKey?: boolean;
			inVertical?: boolean;
			forward?: boolean;
			trap?: boolean;
		}
	): boolean {
		const { metaKey, inVertical, forward = true, trap } = options;
		switch (action) {
			case 'tab':
				if (this.currentElement) {
					focusNextTabIndex({ container: this.currentElement, forward });
				}
				return false; // do not toggle outline

			case 'left':
				if (trap) return true;
				if (inVertical) {
					this.focusNextVertical(false);
				} else if (!this.focusSibling({ forward: false, metaKey })) {
					this.focusParent(false);
				}
				break;

			case 'right':
				if (trap) return true;
				if (inVertical) {
					this.focusNextVertical(true);
				} else if (!this.focusSibling({ forward: true, metaKey })) {
					this.focusChild();
				}
				break;

			case 'up':
				if (trap) return true;
				if (!this.focusSibling({ forward: false, metaKey })) {
					this.focusParent(false);
				}
				break;

			case 'down':
				if (trap) return true;
				if (inVertical) {
					if (!this.focusSibling({ forward: true, metaKey })) {
						if (!this.focusChild()) {
							this.focusParent(true);
						}
					}
					break;
				}
				if (!this.focusChild()) {
					this.focusParent(true);
				}
				break;
		}
		return true;
	}

	/**
	 * Caches the active child index for all parent focusables.
	 * This allows us to remember which child was last active when returning to any container.
	 *
	 * @param element - The currently active element
	 */
	private cacheActiveIndex(element: HTMLElement) {
		let currentElement = element;
		const metadata = this.getMetadata(currentElement);

		// Traverse up the full hierarchy and cache indices for all parents
		let parentElement = metadata?.parentElement;
		while (parentElement) {
			const parentMetadata = this.getMetadata(parentElement);

			// Cache index for all parents (not just vertical containers)
			if (parentMetadata) {
				const childIndex = parentMetadata.children.indexOf(currentElement);
				if (childIndex >= 0) {
					this.indexCache.set(parentElement, childIndex);
				}
			}

			// Move up to the next parent
			currentElement = parentElement;
			parentElement = parentMetadata?.parentElement;
		}
	}

	/**
	 * Navigates to the next or previous vertical container by traversing up through non-vertical parent containers,
	 * searching for sibling containers in the specified direction at each level. If no vertical container is found
	 * at one level, continues searching at higher non-vertical parent levels. This ensures navigation between
	 * major sections while allowing traversal across multiple hierarchy levels. Uses deep cached hierarchy
	 * traversal to focus the exact position where you were last working.
	 *
	 * @param forward - Whether to search forward (true) or backward (false) for the next vertical container
	 * @returns True if a vertical container was found and focused, false otherwise
	 */
	focusNextVertical(forward: boolean): boolean {
		if (!this.currentElement) return false;

		let searchElement = this.currentElement;
		let depth = 0;

		while (searchElement && depth < MAX_TRAVERSAL_DEPTH) {
			// Find non-list parent container
			const nonVerticalParent = this.findNonVerticalParent(searchElement);
			if (!nonVerticalParent) return false;

			const parentMetadata = this.getMetadata(nonVerticalParent);
			if (!parentMetadata) return false;

			// Find the current branch from the non-list parent's perspective
			const currentBranch = this.findCurrentBranchFromParent(searchElement, nonVerticalParent);
			if (!currentBranch) return false;

			// Find siblings of the current branch
			const siblings = parentMetadata.children;
			const currentIndex = siblings.indexOf(currentBranch);

			if (currentIndex !== -1) {
				// Get siblings in the search direction
				const searchSiblings = this.getSiblingsInDirection(siblings, currentBranch, forward);

				// Search through siblings for one that contains a vertical container
				for (const sibling of searchSiblings) {
					const verticalElement = this.findVerticalDescendant(sibling, forward);
					if (verticalElement) {
						// Found a vertical container, traverse to deepest cached descendant
						const deepestTarget = this.getDeepestCachedDescendant(verticalElement);
						return this.activateAndFocus(deepestTarget);
					}
				}
			}

			// No vertical container found at this level, continue traversing up
			// Set the non-vertical parent as the new search element for the next iteration
			searchElement = nonVerticalParent;
			depth++;
		}

		return false;
	}

	/**
	 * Traverses up the hierarchy to find the first parent that is not a vertical container.
	 * This ensures we navigate between major sections rather than intermediate verticals.
	 *
	 * @param startElement - The element to start searching from
	 * @returns The first non-list parent, or undefined if none found
	 */
	private findNonVerticalParent(startElement: HTMLElement): HTMLElement | undefined {
		let current = startElement;
		let depth = 0;

		while (current && depth < MAX_TRAVERSAL_DEPTH) {
			const metadata = this.getMetadata(current);
			const parentElement = metadata?.parentElement;

			if (!parentElement) {
				// No parent - return current if it's not a list, otherwise null
				return metadata?.options.vertical ? undefined : current;
			}

			const parentMetadata = this.getMetadata(parentElement);
			if (!parentMetadata) {
				// Parent exists but not registered - move up
				current = parentElement;
				depth++;
				continue;
			}

			// If parent is not vertical, we found our non-vertical parent
			if (!parentMetadata.options.vertical) {
				return parentElement;
			}

			// Parent is a list, continue up
			current = parentElement;
			depth++;
		}

		return undefined;
	}

	/**
	 * Finds which child of the non-list parent contains the current element.
	 * This helps us identify the "branch" we're currently in.
	 *
	 * @param currentElement - The element we're currently focused on
	 * @param nonVerticalParent - The non-vertical parent container
	 * @returns The direct child of nonVerticalParent that contains currentElement
	 */
	private findCurrentBranchFromParent(
		currentElement: HTMLElement,
		nonVerticalParent: HTMLElement
	): HTMLElement | undefined {
		const parentMetadata = this.getMetadata(nonVerticalParent);
		if (!parentMetadata) return undefined;

		// Check each child of the non-list parent to see which one contains our current element
		for (const child of parentMetadata.children) {
			if (this.containsElement(child, currentElement)) {
				return child;
			}
		}

		return undefined;
	}

	/**
	 * Checks if a container element contains a target element anywhere in its hierarchy.
	 *
	 * @param container - The container element to search within
	 * @param target - The target element to find
	 * @returns True if container contains target
	 */
	private containsElement(container: HTMLElement, target: HTMLElement): boolean {
		if (container === target) return true;

		const containerMetadata = this.getMetadata(container);
		if (!containerMetadata) return false;

		// Recursively check children
		for (const child of containerMetadata.children) {
			if (this.containsElement(child, target)) {
				return true;
			}
		}

		return false;
	}

	/**
	 * Recursively searches within an element and its descendants for a vertical element.
	 * A vertical element is one that has `vertical: true` in its FocusableOptions.
	 *
	 * @param element - The element to search within
	 * @param forward - Whether to search in forward order (affects which vertical element is returned if multiple exist)
	 * @returns The first vertical element found, or undefined if none found
	 */
	private findVerticalDescendant(element: HTMLElement, forward: boolean): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata) return undefined;

		if (!this.shouldSkipElement(metadata)) {
			return element;
		}

		// Check if this element itself is vertical
		if (metadata.options.vertical) {
			return element;
		}

		// Search through children
		const children = this.getChildrenInDirection(metadata.children, forward);

		for (const child of children) {
			const result = this.findVerticalDescendant(child, forward);
			if (result) {
				return result;
			}
		}

		return undefined;
	}

	getOptions(element?: HTMLElement): FocusableOptions | null {
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
		const isCurrent = this.currentElement === element;
		const marker = isCurrent ? ' ◀── CURRENT' : '';

		// Build status indicators
		const flags: string[] = [];
		if (metadata.options.disabled) flags.push('disabled');
		if (metadata.options.trap) flags.push('trap');
		if (metadata.options.vertical) flags.push('vertical');
		if (metadata.options.isolate) flags.push('isolate');
		if (this.shouldSkipElement(metadata)) flags.push('skip');
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

	/**
	 * Focuses the nth item in the parent of the current element.
	 */
	focusNthSibling(n: number): boolean {
		const currentMetadata = this.getCurrentMetadata();
		if (!currentMetadata?.parentElement) return false;

		const metadata = this.getMetadata(currentMetadata.parentElement);
		if (!metadata) return false;

		if (n >= metadata.children.length) return false;
		return this.activateAndFocus(metadata.children[n]);
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
