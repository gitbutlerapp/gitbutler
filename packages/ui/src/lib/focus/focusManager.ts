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

type NavigationAction = 'tab' | 'left' | 'right' | 'up' | 'down';

export type KeyboardHandler = (event: KeyboardEvent) => boolean | void;

export interface FocusableOptions {
	// Identifier for this focusable element, used by custom navigation code
	id?: DefinedFocusable;
	// Custom tab order within siblings (overrides default DOM order)
	tabIndex?: number;
	// Keep focusable inactive and outside navigation hierarchy
	disabled?: boolean;
	// Treat children as a list (changes arrow key behavior, automatically sets skip: true)
	list?: boolean;
	// Prevent keyboard navigation from leaving this element
	trap?: boolean;
	// Automatically focus this element when registered
	activate?: boolean;
	// Don't establish parent-child relationships with other focusables
	isolate?: boolean;
	// Link to other focusable types for boundary navigation
	linkToIds?: DefinedFocusable[];
	// Skip this element in navigation, go directly to first child or parent
	skip?: boolean;
	// Never highlight the element
	dim?: boolean;

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

	// Cache for last active index of list focusables (element -> last active child index)
	private _listFocusableCache = new Map<HTMLElement, number>();

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

	private getRightValidPreviousElement(): HTMLElement | undefined {
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
	 * Gets siblings of an element in a specified direction, excluding the element itself.
	 * @param siblings - Array of sibling elements
	 * @param currentElement - The element to exclude from siblings
	 * @param forward - Whether to get siblings after (true) or before (false) the current element
	 * @returns Array of siblings in the specified direction, reversed if searching backward
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
	 * @param children - Array of child elements
	 * @param forward - Whether to iterate forward (true) or backward (false)
	 * @returns Array of children in the specified order
	 */
	private getChildrenInDirection(children: HTMLElement[], forward: boolean): HTMLElement[] {
		return forward ? children : children.slice().reverse();
	}

	/**
	 * Gets the parent element's metadata for a given metadata object.
	 * @param metadata - The metadata to get the parent from
	 * @returns Parent metadata or undefined if no parent exists
	 */
	private getParentMetadata(metadata: FocusableData | undefined): FocusableData | undefined {
		if (!metadata?.parentElement) return undefined;
		return this.getMetadata(metadata.parentElement);
	}

	/**
	 * Normalizes focusable options by applying automatic defaults.
	 * When list: true is specified, automatically sets skip: true unless explicitly overridden.
	 * @param options - The raw focusable options
	 * @returns Normalized options with automatic defaults applied
	 */
	private normalizeFocusableOptions(options: FocusableOptions): FocusableOptions {
		// If list: true and skip is not explicitly defined, set skip: true
		if (options.list && options.skip === undefined) {
			return { ...options, skip: true };
		}
		return options;
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

		const normalizedOptions = this.normalizeFocusableOptions(options);
		const { id: logicalId } = normalizedOptions;
		this.unregisterElement(element);

		const parentElement = normalizedOptions.isolate ? undefined : this.findParent(element);
		const parentMeta = parentElement ? this.getMetadata(parentElement) : undefined;

		const metadata: FocusableData = {
			logicalId,
			parentElement,
			children: [],
			options: normalizedOptions
		};

		this.establishParentChildRelationships(element, metadata, parentMeta);
		this.metadata.set(element, metadata);
		this.resolvePendingRelationships();
		this.handlePendingRegistration(element, parentMeta, normalizedOptions);

		if (normalizedOptions.activate) {
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
		removeFromArray(this._previousElements, element);

		// Clean up cache entry for this element
		this._listFocusableCache.delete(element);

		// Clear current if it matches
		if (this._currentElement === element) {
			const rightElement = this.getRightValidPreviousElement();
			if (rightElement) {
				this._currentElement = rightElement;
				// Remove the selected element from history since it's now current
				removeFromArray(this._previousElements, rightElement);
			} else {
				this._currentElement = undefined;
			}
			this.cursor.set(this._currentElement);
		}
	}

	/**
	 * Fires onActive callbacks for an element and all its parent focusables
	 */
	private fireOnActiveForHierarchy(element: HTMLElement, active: boolean): void {
		let metadata = this.getMetadata(element);
		while (metadata) {
			try {
				metadata.options.onActive?.(active);
			} catch (error) {
				console.warn(`Error in onActive(${active}) callback:`, error);
			}
			const parentElement = metadata.parentElement;
			if (!parentElement) break;
			metadata = this.getMetadata(parentElement);
		}
	}

	/**
	 * Sets the specified element as the currently active (focused) element.
	 * If the element has skip=true, it will try to focus the preferred child instead.
	 * Triggers onBlur callback for the previously active element and onFocus
	 * callback for the newly active element. Also fires onActive callbacks
	 * for all parent focusables. Updates focus history and caches the active index
	 * for list focusables.
	 */
	setActive(element: HTMLElement) {
		if (!element || !element.isConnected || !this.isElementRegistered(element)) {
			return;
		}

		const metadata = this.getMetadata(element);

		// If element should be skipped, try to focus preferred child (cached or first) instead
		if (metadata?.options.skip) {
			const preferredChild = this.getPreferredChildFromContainer(element);
			if (preferredChild) {
				this.setActive(preferredChild);
				return;
			}
			// If no children, don't set this element as active
			return;
		}

		const previousElement = this._currentElement;

		// Fire onActive(false) for previous element and all its parents
		if (previousElement) {
			this.fireOnActiveForHierarchy(previousElement, false);
		}

		// Cache the active child index for all parent elements
		this.cacheActiveIndexForAllParents(element);

		// Add current element to history before changing
		if (this._currentElement) {
			this.addToPreviousElements(this._currentElement);
		}
		this._currentElement = element;

		// Fire onActive(true) for new element and all its parents
		this.fireOnActiveForHierarchy(element, true);

		this.cursor.set(element);
	}

	/**
	 * Recursively searches for the right focusable element of specific types
	 * by traversing up the hierarchy and checking sibling branches.
	 * Finds the nearest element of any of the specified types.
	 *
	 * @param searchElement - Current element to search from
	 * @param searchTypes - Types of focusable elements to find (can be single type or array)
	 * @param forward - Whether to search forward or backward through siblings
	 * @returns The right matching element or undefined if none found
	 */
	private findNext(
		searchElement: HTMLElement,
		searchTypes: DefinedFocusable | DefinedFocusable[],
		forward: boolean
	): HTMLElement | undefined {
		const currentMeta = this.getMetadata(searchElement);
		const parentMeta = this.getParentMetadata(currentMeta);
		if (!parentMeta || !this._currentElement) return;

		const typesToSearch = this.normalizeSearchTypes(searchTypes);
		const childrenToSearch = this.getSiblingsInDirection(
			parentMeta.children,
			searchElement,
			forward
		);

		for (const nextChild of childrenToSearch) {
			const result = this.findWithin(nextChild, typesToSearch, forward);
			if (result) return result;
		}

		if (!currentMeta?.parentElement) return undefined;
		return this.findNext(currentMeta.parentElement, searchTypes, forward);
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
		const children = this.getChildrenInDirection(metadata.children, forward);

		for (const child of children) {
			const childMeta = this.getMetadata(child);

			// Skip elements with skip=true unless they match the search types
			if (childMeta?.options.skip) {
				// Only consider skipped elements if they match the search types
				if (childMeta.logicalId && typesToSearch.includes(childMeta.logicalId)) {
					// Even if it matches, prefer its children if any exist
					const firstGrandChild = childMeta.children.at(0);
					if (firstGrandChild) {
						const grandChildResult = this.findWithin(child, searchTypes, forward);
						if (grandChildResult) return grandChildResult;
					}
					return child;
				}
				// Search within skipped elements but don't return them directly
				const subchild = this.findWithin(child, searchTypes, forward);
				if (subchild) {
					return subchild;
				}
				continue;
			}

			if (childMeta?.logicalId && typesToSearch.includes(childMeta.logicalId)) {
				return child;
			}
			const subchild = this.findWithin(child, searchTypes, forward);
			if (subchild) {
				return subchild;
			}
		}
	}

	/**
	 * Gets the preferred child element to focus within any container.
	 * Uses cached index if available, otherwise falls back to first child.
	 *
	 * @param containerElement - The container element to get a child from
	 * @returns The preferred child element to focus, or undefined if no children
	 */
	private getPreferredChildFromContainer(containerElement: HTMLElement): HTMLElement | undefined {
		const containerMetadata = this.getMetadata(containerElement);
		if (!containerMetadata?.children.length) {
			return undefined;
		}

		// Check if we have a cached index for this container
		const cachedIndex = this._listFocusableCache.get(containerElement);
		if (cachedIndex !== undefined && cachedIndex < containerMetadata.children.length) {
			const cachedChild = containerMetadata.children[cachedIndex];
			if (cachedChild && cachedChild.isConnected) {
				return cachedChild;
			}
		}

		// Fall back to first child if no cache or cache is invalid
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

			// Get the preferred child from current container
			const preferredChild = this.getPreferredChildFromContainer(current);
			if (!preferredChild) {
				// No children available, return current container
				return current;
			}

			// Check if the preferred child has cached state
			const childHasCache = this._listFocusableCache.has(preferredChild);

			if (!childHasCache) {
				// No cache - this is our final target
				return preferredChild;
			}

			// Child has cache - continue traversing deeper
			current = preferredChild;
		}

		// Fallback: return the last container we were examining
		return current;
	}

	/**
	 * Recursively searches for the next non-skipped focusable element within list contexts.
	 * Traverses up the hierarchy while parent elements have list: true, ensuring navigation
	 * stays within the same column/list structure.
	 *
	 * @param searchElement - Current element to search from
	 * @param forward - Whether to search forward (next) or backward (previous) through siblings
	 * @returns The next non-skipped focusable element or undefined if none found
	 */
	private findNextNonSkippedInList(
		searchElement: HTMLElement,
		forward: boolean
	): HTMLElement | undefined {
		const currentMeta = this.getMetadata(searchElement);
		const parentMeta = this.getParentMetadata(currentMeta);
		if (!currentMeta || !parentMeta || !this._currentElement) return undefined;

		// If we reach an element that is not a list we want to exit, as to stay
		// in the same column.
		// console.log({ parentMeta, currentMeta });
		if (!parentMeta.options.list) {
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

		// Only continue traversing up if the parent has list: true
		if (parentMeta.options.list && currentMeta.parentElement) {
			return this.findNextNonSkippedInList(currentMeta.parentElement, forward);
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

			// Skip elements with skip=true entirely
			if (childMeta?.options.skip) {
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

	private activateAndFocus(element: HTMLElement | undefined): boolean {
		if (!element) return false;
		this.setActive(element);
		this.focusElement(element);
		return true;
	}

	/**
	 * Navigates to the right or previous sibling element. If no sibling exists
	 * and we're at a list boundary with linked types configured, attempts
	 * boundary navigation to linked element types.
	 *
	 * @param forward - Whether to navigate to right (true) or previous (false) sibling
	 * @returns True if navigation succeeded, false otherwise
	 */
	focusSibling(options: { forward: boolean; metaKey: boolean }): boolean {
		const { forward, metaKey: meta } = options;
		const currentMetadata = this.getCurrentMetadata();
		const parentMetadata = this.getParentMetadata(currentMetadata);
		if (!parentMetadata || !this._currentElement) return false;

		const siblings = parentMetadata.children;
		const currentIndex = siblings.indexOf(this._currentElement);
		const isListWithLinks = currentMetadata?.options.linkToIds;

		if (isListWithLinks && meta) {
			return this.focusColumnSibling(forward);
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

		if (isAtBoundary) {
			return this.focusColumnSibling(forward);
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
	 * Finds the next non-skipped focusable, traversing parents while list: true.
	 *
	 * @param forward - Whether to search forward or backward in the tree
	 * @returns True if a linked element was found and focused, false if no focusable was found
	 */
	focusColumnSibling(forward: boolean): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata?.parentElement || !this._currentElement) return false;

		// Find the next non-skipped focusable, traversing parents while list: true
		const linkedTarget = this.findNextNonSkippedInList(this._currentElement, forward);
		if (!linkedTarget) return false;

		return this.activateAndFocus(linkedTarget);
	}

	focus(forward: boolean): boolean {
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

	focusParent(forward: boolean = true): void {
		const currentMetadata = this.getCurrentMetadata();
		if (!currentMetadata?.parentElement || !this._currentElement) return;

		let currentElement = this._currentElement;
		let parentElement = currentMetadata.parentElement;
		let parentMetadata = this.getMetadata(parentElement);
		let depth = 0;

		while (parentElement && parentMetadata && depth < MAX_TRAVERSAL_DEPTH) {
			// If parent has skip=true, try to find non-skippable descendant in the specified direction
			if (parentMetadata.options.skip) {
				// Only look for focusable descendents a skipped focusable is of list type,
				// otherwise keep traversing the parent elements.
				if (parentMetadata.options.list) {
					const siblingsToSearch = this.getSiblingsInDirection(
						parentMetadata.children,
						currentElement,
						forward
					);

					for (const sibling of siblingsToSearch) {
						const nonSkippableDescendant = this.findNonSkippedDescendant(sibling, forward);
						if (nonSkippableDescendant && nonSkippableDescendant !== sibling) {
							this.activateAndFocus(nonSkippableDescendant);
							return;
						}
						// If sibling itself is non-skippable, focus it
						const siblingMeta = this.getMetadata(sibling);
						if (!siblingMeta?.options.skip) {
							this.activateAndFocus(sibling);
							return;
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
	 * If the preferred child has skip=true, it will try to focus its preferred child instead.
	 */
	focusChild(): boolean {
		const metadata = this.getCurrentMetadata();
		if (!metadata || !this._currentElement) return false;

		// Get the preferred child (cached or first child)
		const preferredChild = this.getPreferredChildFromContainer(this._currentElement);
		if (!preferredChild) return false;

		const childMetadata = this.getMetadata(preferredChild);

		// If the preferred child should be skipped, try to focus its preferred child (cached or first)
		if (childMetadata?.options.skip) {
			const preferredGrandChild = this.getPreferredChildFromContainer(preferredChild);
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
			inList: navigationContext.inList
		});
	}

	private buildNavigationContext(event: KeyboardEvent, metadata: FocusableData) {
		const parentElement = metadata?.parentElement;
		const parentData = parentElement ? this.getMetadataOrThrow(parentElement) : undefined;
		const isList = metadata.options.list ?? false;
		const inList = parentData?.options.list ?? false;
		const action = this.getNavigationAction(event.key);

		return {
			action,
			isList,
			inList,
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
	 * - Right/Down: Navigate to right item or enter child
	 *
	 * In non-list contexts:
	 * - Left/Up: Navigate to parent or previous sibling
	 * - Right/Down: Navigate to child or right sibling
	 *
	 * @param key - The keyboard key that was pressed
	 * @param inList - Whether the current element is in a list context
	 * @returns The navigation action to perform, or null if key is not handled
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
			inList: boolean;
			forward: boolean;
			trap?: boolean;
		}
	): boolean {
		const { metaKey, inList, forward = true, trap } = options;
		switch (action) {
			case 'tab':
				if (this._currentElement) {
					focusNextTabIndex({ container: this._currentElement, forward });
				}
				return false; // do not toggle outline

			case 'left':
				if (trap) return true;
				if (inList) {
					this.focusNextList(false);
				} else if (!this.focusSibling({ forward: false, metaKey })) {
					this.focusParent(false);
				}
				break;

			case 'right':
				if (trap) return true;
				if (inList) {
					this.focusNextList(true);
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
				if (inList) {
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
	private cacheActiveIndexForAllParents(element: HTMLElement) {
		let currentElement = element;
		const metadata = this.getMetadata(currentElement);

		// Traverse up the full hierarchy and cache indices for all parents
		let parentElement = metadata?.parentElement;
		while (parentElement) {
			const parentMetadata = this.getMetadata(parentElement);

			// Cache index for all parents (not just lists)
			if (parentMetadata) {
				const childIndex = parentMetadata.children.indexOf(currentElement);
				if (childIndex >= 0) {
					this._listFocusableCache.set(parentElement, childIndex);
				}
			}

			// Move up to the next parent
			currentElement = parentElement;
			parentElement = parentMetadata?.parentElement;
		}
	}

	/**
	 * Navigates to the next or previous list by traversing up through non-list parent containers,
	 * searching for sibling containers in the specified direction at each level. If no list is found
	 * at one level, continues searching at higher non-list parent levels. This ensures navigation between
	 * major sections while allowing traversal across multiple hierarchy levels. Uses deep cached hierarchy
	 * traversal to focus the exact position where you were last working.
	 *
	 * @param forward - Whether to search forward (true) or backward (false) for the next list
	 * @returns True if a list was found and focused, false otherwise
	 */
	focusNextList(forward: boolean): boolean {
		if (!this._currentElement) return false;

		let searchElement = this._currentElement;
		let depth = 0;

		while (searchElement && depth < MAX_TRAVERSAL_DEPTH) {
			// Find non-list parent container
			const nonListParent = this.findNonListParent(searchElement);
			if (!nonListParent) return false;

			const parentMetadata = this.getMetadata(nonListParent);
			if (!parentMetadata) return false;

			// Find the current branch from the non-list parent's perspective
			const currentBranch = this.findCurrentBranchFromParent(searchElement, nonListParent);
			if (!currentBranch) return false;

			// Find siblings of the current branch
			const siblings = parentMetadata.children;
			const currentIndex = siblings.indexOf(currentBranch);

			if (currentIndex !== -1) {
				// Get siblings in the search direction
				const searchSiblings = this.getSiblingsInDirection(siblings, currentBranch, forward);

				// Search through siblings for one that contains a list
				for (const sibling of searchSiblings) {
					const listElement = this.findListDescendant(sibling, forward);
					if (listElement) {
						// Found a list, traverse to deepest cached descendant
						const deepestTarget = this.getDeepestCachedDescendant(listElement);
						return this.activateAndFocus(deepestTarget);
					}
				}
			}

			// No list found at this level, continue traversing up
			// Set the non-list parent as the new search element for the next iteration
			searchElement = nonListParent;
			depth++;
		}

		return false;
	}

	/**
	 * Traverses up the hierarchy to find the first parent that is not a list container.
	 * This ensures we navigate between major sections rather than intermediate lists.
	 *
	 * @param startElement - The element to start searching from
	 * @returns The first non-list parent, or undefined if none found
	 */
	private findNonListParent(startElement: HTMLElement): HTMLElement | undefined {
		let current = startElement;
		let depth = 0;

		while (current && depth < MAX_TRAVERSAL_DEPTH) {
			const metadata = this.getMetadata(current);
			const parentElement = metadata?.parentElement;

			if (!parentElement) {
				// No parent - return current if it's not a list, otherwise null
				return metadata?.options.list ? undefined : current;
			}

			const parentMetadata = this.getMetadata(parentElement);
			if (!parentMetadata) {
				// Parent exists but not registered - move up
				current = parentElement;
				depth++;
				continue;
			}

			// If parent is not a list, we found our non-list parent
			if (!parentMetadata.options.list) {
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
	 * @param nonListParent - The non-list parent container
	 * @returns The direct child of nonListParent that contains currentElement
	 */
	private findCurrentBranchFromParent(
		currentElement: HTMLElement,
		nonListParent: HTMLElement
	): HTMLElement | undefined {
		const parentMetadata = this.getMetadata(nonListParent);
		if (!parentMetadata) return undefined;

		// Check each child of the non-list parent to see which one contains our current element
		for (const child of parentMetadata.children) {
			if (this.elementContains(child, currentElement)) {
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
	private elementContains(container: HTMLElement, target: HTMLElement): boolean {
		if (container === target) return true;

		const containerMetadata = this.getMetadata(container);
		if (!containerMetadata) return false;

		// Recursively check children
		for (const child of containerMetadata.children) {
			if (this.elementContains(child, target)) {
				return true;
			}
		}

		return false;
	}

	/**
	 * Recursively searches within an element and its descendants for a list element.
	 * A list element is one that has `list: true` in its FocusableOptions.
	 *
	 * @param element - The element to search within
	 * @param forward - Whether to search in forward order (affects which list is returned if multiple exist)
	 * @returns The first list element found, or undefined if none found
	 */
	private findListDescendant(element: HTMLElement, forward: boolean): HTMLElement | undefined {
		const metadata = this.getMetadata(element);
		if (!metadata) return undefined;

		if (!metadata.options.skip || metadata.children.length === 0) {
			return element;
		}

		// Check if this element itself is a list
		if (metadata.options.list) {
			return element;
		}

		// Search through children
		const children = this.getChildrenInDirection(metadata.children, forward);

		for (const child of children) {
			const result = this.findListDescendant(child, forward);
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
		const isCurrent = this._currentElement === element;
		const marker = isCurrent ? ' ◀── CURRENT' : '';

		// Build status indicators
		const flags: string[] = [];
		if (metadata.logicalId) flags.push(`id:${metadata.logicalId}`);
		if (metadata.options.disabled) flags.push('disabled');
		if (metadata.options.trap) flags.push('trap');
		if (metadata.options.list) flags.push('list');
		if (metadata.options.isolate) flags.push('isolate');
		if (metadata.options.skip) flags.push('skip');
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
