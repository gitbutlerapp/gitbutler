import { FModeManager } from '$lib/focus/fModeManager';
import { getNavigationAction, isInputElement, getElementDescription } from '$lib/focus/focusUtils';
import { focusNextTabIndex } from '$lib/focus/tabbable';
import { removeFromArray, scrollIntoViewIfNeeded } from '$lib/focus/utils';
import { parseHotkey, matchesHotkey } from '$lib/utils/hotkeySymbols';
import { mergeUnlisten } from '$lib/utils/mergeUnlisten';
import { InjectionToken } from '@gitbutler/core/context';
import { on } from 'svelte/events';
import { writable } from 'svelte/store';
import type {
	FocusableNode,
	FocusableOptions,
	KeyboardHandler,
	NavigationContext,
	NavigationAction
} from '$lib/focus/focusTypes';
export type { FocusableOptions } from './focusTypes';

export const FOCUS_MANAGER: InjectionToken<FocusManager> = new InjectionToken('FocusManager');

const MAX_HISTORY_SIZE = 10;
const MAX_PARENT_SEARCH_DEPTH = 100;

export class FocusManager {
	// ============================================
	// Properties
	// ============================================

	private nodeMap = new Map<HTMLElement, FocusableNode>();
	private currentNode: FocusableNode | undefined;
	private pendingRelationships: HTMLElement[] = [];

	private previousElements: HTMLElement[] = [];
	private indexCache = new Map<HTMLElement, number>();
	private fModeManager: FModeManager;

	readonly cursor = writable<HTMLElement | undefined>();
	readonly outline = writable(false); // external use
	private _outline = false; // internal use

	private handleMouse = this.handleClick.bind(this);
	private handleKeys = this.handleKeydown.bind(this);

	constructor() {
		this.fModeManager = new FModeManager();
	}

	setFModeEnabled(enabled: boolean): void {
		this.fModeManager.setFeatureEnabled(enabled);
	}

	private setOutline(value: boolean): void {
		this._outline = value;
		this.outline.set(value);
	}

	// ============================================
	// CORE EVENT HANDLERS (Most Important)
	// ============================================

	// Handles keyboard navigation with arrow keys, Tab, and custom handlers
	private handleKeydown(event: KeyboardEvent) {
		if (this.processKeyboardEvent(event)) {
			event.stopPropagation();
			event.preventDefault();
		}
	}

	private processKeyboardEvent(event: KeyboardEvent): boolean {
		if (this.shouldSkipEvent(event)) return false;
		if (this.handleFModeInput(event)) return true;
		if (this.handleHotkeyPress(event)) return true;

		const node = this.updateCurrentNode(event);
		if (!node) return false;

		const context = this.buildNavigationContext(event, node);

		if (this.handleTabKey(context, event)) return true;
		if (this.handleEscapeKey(event)) return true;
		if (this.hasSelection()) return false;
		if (this.handleOutlineDisplay(context)) return true;
		if (this.handleActions(event)) return true;

		return this.handleNavigation(event, context);
	}

	// Handles mouse clicks to update focus, traversing up to find focusables
	private handleClick(e: MouseEvent) {
		// Ignore keyboard initiated clicks.
		if (e.detail === 0) {
			return;
		}

		if (e.target instanceof HTMLElement) {
			const focusableNode = this.findNearestFocusableElement(e.target);
			if (focusableNode) {
				this.setActiveNode(focusableNode);
				this.setOutline(false);
			}
		}

		// TODO: Find a way to update the focusable without causing target to blur.
		if (isInputElement(e.target)) {
			e.target.focus();
		}
	}

	// Handles hotkeys for instant button activation (supports complex combinations like ⇧⌘P)
	private handleHotkeyPress(event: KeyboardEvent): boolean {
		// Skip if just pressing modifier keys alone
		if (['Meta', 'Control', 'Alt', 'Shift'].includes(event.key)) return false;

		// Find all buttons with hotkeys
		const entries = Array.from(this.nodeMap.entries());
		for (const [element, node] of entries) {
			if (node.options.button && node.options.hotkey) {
				// Parse the hotkey definition
				const parsed = parseHotkey(node.options.hotkey);
				if (!parsed) continue;

				// Check if the event matches the hotkey
				if (matchesHotkey(event, parsed)) {
					event.preventDefault();
					event.stopPropagation();

					// Trigger click on the button
					try {
						element.click();
					} catch (error) {
						console.warn('Error triggering button click via hotkey:', error);
					}
					return true;
				}
			}
		}
		return false;
	}

	// ============================================
	// Public API
	// ============================================

	listen() {
		return mergeUnlisten(
			on(document, 'click', this.handleMouse, { capture: true }),
			on(document, 'keydown', this.handleKeys)
		);
	}

	// Handles deferred linking when parents register after children
	register(options: FocusableOptions, element: HTMLElement) {
		if (!element || !element.isConnected) {
			console.warn('Attempted to register invalid or disconnected element');
			return;
		}

		this.unregisterElement(element);

		const parentNode = options.isolate ? undefined : this.findParentNode(element);

		const newNode: FocusableNode = {
			element,
			parent: parentNode,
			children: [],
			options
		};

		this.nodeMap.set(element, newNode);
		this.establishParentChildRelationships(element, newNode, parentNode);
		this.resolvePendingRelationships();
		this.handlePendingRegistration(element, parentNode, options);

		if (this.fModeManager.active) {
			this.fModeManager.addElement(element, newNode);
		}

		if (options.activate) {
			this.setActiveNode(newNode);
		}
	}

	unregister(element: HTMLElement) {
		// Always remove from F-mode manager to clean up any stored shortcuts
		this.fModeManager.removeElement(element);

		this.unregisterElement(element);

		this.pendingRelationships = this.pendingRelationships.filter((p) => p !== element);
		removeFromArray(this.previousElements, element);
		this.indexCache.delete(element);

		if (this.currentNode?.element === element) {
			const previousElement = this.getValidPreviousElement();
			if (previousElement) {
				this.setActiveNode(this.nodeMap.get(previousElement));
				removeFromArray(this.previousElements, previousElement);
			} else {
				this.clearCurrent();
			}
			this.cursor.set(this.currentNode?.element);
		}
	}

	// ============================================
	// NAVIGATION METHODS
	// ============================================

	private clearCurrent() {
		this.currentNode = undefined;
		this.cursor.set(undefined);
	}

	private setActiveNode(node?: FocusableNode): boolean {
		if (!node) return false;
		const previousNode = this.currentNode;

		if (!node.options.focusable) return false;

		this.focusElement(node.element);
		this.cursor.set(node.element);

		if (node === this.currentNode) return true;

		if (previousNode) {
			this.triggerOnActiveCallbacksNode(previousNode, false);
			this.addToPreviousElements(previousNode.element);
		}

		this.currentNode = node;
		this.cacheActiveIndexNode(node);
		this.triggerOnActiveCallbacksNode(node, true);
		return true;
	}

	focusSibling(options: { forward: boolean; metaKey?: boolean }): boolean {
		const { forward } = options;
		const currentNode = this.currentNode;
		if (!currentNode) return false;

		const parentNode = this.getParentNode(currentNode);
		if (!parentNode) return false;

		const navigableSiblings = this.getNavigableChildNodes(parentNode);
		const currentIndex = navigableSiblings.findIndex((n) => n.element === currentNode.element);

		if (currentIndex === -1) return false;

		const targetIndex = forward ? currentIndex + 1 : currentIndex - 1;
		const hasValidSibling = targetIndex >= 0 && targetIndex < navigableSiblings.length;

		if (hasValidSibling) {
			return this.setActiveNode(this.findNavigableDescendant(navigableSiblings[targetIndex]));
		}

		const isAtBoundary =
			(forward && currentIndex === navigableSiblings.length - 1) ||
			(!forward && currentIndex === 0);

		if (isAtBoundary) {
			// Traverse parents while vertical: true
			const linkedTarget = this.findNextInColumnNode(currentNode, forward);
			if (!linkedTarget) return false;
			return this.setActiveNode(linkedTarget);
		}

		return false;
	}

	focusNextVertical(forward: boolean): boolean {
		if (!this.currentNode?.element) return false;
		const nextChild = this.findInNextColumn(this.currentNode.element, forward);
		if (nextChild) {
			return this.setActiveNode(nextChild);
		}
		return false;
	}

	focusNthSibling(n: number): boolean {
		if (!this.currentNode?.parent) return false;

		const navigableChildren = this.getNavigableChildNodes(this.currentNode.parent);
		const child = navigableChildren.at(n);
		return this.setActiveNode(child);
	}

	updateElementOptions(element: HTMLElement, updates: Partial<FocusableOptions>): boolean {
		const node = this.getNode(element);
		if (!node) return false;
		node.options = { ...node.options, ...updates };
		return true;
	}

	// ============================================
	// DEBUGGING UTILITIES
	// ============================================

	debugPrintTree(logElements: boolean): void {
		const rootElements: HTMLElement[] = [];
		Array.from(this.nodeMap.entries()).forEach(([element, node]) => {
			if (!node.parent) {
				rootElements.push(element);
			}
		});

		if (rootElements.length === 0) {
			return;
		}

		for (const root of rootElements) {
			this.printTreeNode(root, logElements, 0);
		}
	}

	// ============================================
	// HELPER METHODS
	// ============================================

	private findNearestFocusableElement(start: HTMLElement): FocusableNode | undefined {
		let pointer: HTMLElement | null = start;
		while (pointer) {
			const node = this.getNode(pointer);
			if (node) {
				if (node.options.focusable) {
					return node;
				}
				const navigableChild = this.findNavigableDescendant(node);
				// Skip button elements - continue traversing up
				if (navigableChild) {
					return navigableChild;
				}
			}
			pointer = pointer.parentElement;
		}
	}

	private getDefaultRoot(): FocusableNode | undefined {
		if (document.activeElement && document.activeElement instanceof HTMLElement) {
			const node = this.findNearestFocusableElement(document.activeElement);
			if (node) return node;
		}

		const firstNode = this.nodeMap.values().next().value;
		if (firstNode) {
			let node: FocusableNode | undefined = firstNode;
			while (node) {
				if (node.options.activate) {
					return node;
				}
				if (!node.parent) {
					return node;
				}
				node = node.parent;
			}
		}
	}

	private shouldSkipEvent(event: KeyboardEvent): boolean {
		return isInputElement(event.target);
	}

	private updateCurrentNode(event: KeyboardEvent): FocusableNode | undefined {
		if (this.currentNode) return this.currentNode;
		if (event.key === 'Tab') return;

		const firstNode = this.findNavigableDescendant(this.getDefaultRoot());
		if (firstNode) {
			this.currentNode = firstNode;
			return firstNode;
		}
	}

	private hasSelection(): boolean {
		const selection = window.getSelection();
		return !!selection && selection.rangeCount > 0 && !selection.isCollapsed;
	}

	private handleActions(event: KeyboardEvent): boolean {
		return this.tryActionHandler(event) || this.tryCustomHandlers(event);
	}

	private handleNavigation(event: KeyboardEvent, context: NavigationContext): boolean {
		if (this.processStandardNavigation(event, context)) {
			this.setOutline(true);
			return true;
		}
		return false;
	}

	private handleFModeInput(event: KeyboardEvent): boolean {
		if (event.key === 'f' || event.key === 'F' || this.fModeManager.active) {
			return this.fModeManager.handleKeypress(event, this.nodeMap, this.currentNode!);
		}
		return false;
	}

	private handleTabKey(navigationContext: NavigationContext, event: KeyboardEvent): boolean {
		if (navigationContext.action !== 'tab') return false;

		if (navigationContext.trap || this._outline) {
			focusNextTabIndex({
				container: this.currentNode!.element,
				forward: !navigationContext.shiftKey,
				trap: navigationContext.trap
			});
			this.setOutline(false);
			if (!navigationContext.trap) {
				// Only clear current node if were tabbing inside of a trap
				this.clearCurrent();
			}
			event.preventDefault();
		}
		return false;
	}

	private handleEscapeKey(event: KeyboardEvent): boolean {
		if (event.key !== 'Escape') return false;

		// Try onEsc handlers first
		if (this.tryEscapeHandlers(event) || this.tryCustomHandlers(event)) {
			return true;
		}
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	private handleOutlineDisplay(navigationContext: NavigationContext): boolean {
		if (!navigationContext.action) return false;
		if (!this.shouldShowOutlineOnly(navigationContext)) return false;

		if (!this.shouldShowOutlineOnly(navigationContext)) return false;
		const targetNode = this.findNavigableDescendant(this.currentNode);
		if (targetNode) {
			this.setActiveNode(targetNode);
			this.setOutline(true);
			return true;
		}
		return false;
	}

	private tryActionHandler(event: KeyboardEvent): boolean {
		if (!this.currentNode) return false;

		// Check if the key is Space or Enter
		if (event.key !== ' ' && event.key !== 'Enter') return false;

		if (!this.currentNode.options.onAction) return false;

		try {
			this.currentNode.options.onAction();
			return true;
		} catch (error) {
			console.warn('Error in onAction handler:', error);
		}

		return false;
	}

	private tryCustomHandlers(event: KeyboardEvent): boolean {
		if (!this.currentNode) return false;

		const keyHandlers = this.getNodeKeydownHandlers(this.currentNode);

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

	// ============================================
	// 7. NAVIGATION CORE
	// ============================================

	// Processes arrow keys and Tab navigation with outline visibility
	private processStandardNavigation(
		event: KeyboardEvent,
		navigationContext: NavigationContext
	): boolean {
		if (!navigationContext.action) return false;

		return this.executeNavigationAction(navigationContext.action, {
			metaKey: event.metaKey,
			trap: navigationContext.trap
		});
	}

	private focusAnyNode() {
		const node = this.findNavigableDescendant(this.currentNode);
		if (node) {
			this.setActiveNode(node);
		}
	}

	// Executes navigation action (tab, arrow keys) with context options
	//
	// Tab uses native navigation, arrows navigate siblings/parents/children.
	// Returns true if outline should be shown.
	private executeNavigationAction(
		action: NavigationAction,
		options: {
			metaKey?: boolean;
			trap?: boolean;
		}
	): boolean {
		const { metaKey, trap } = options;
		switch (action) {
			case 'left':
				if (trap) return true;
				if (!this.focusNextVertical(false)) {
					this.focusAnyNode();
				}
				return true;

			case 'right':
				if (trap) return true;
				if (!this.focusNextVertical(true)) {
					this.focusAnyNode();
				}
				return true;

			case 'up':
				if (trap) return true;
				this.focusSibling({ forward: false, metaKey });
				return true;

			case 'down':
				if (trap) return true;
				this.focusSibling({ forward: true, metaKey });
				return true;
		}
		return false;
	}

	// Finds first navigable descendant, skipping buttons and containers
	private findNavigableDescendant(
		node?: FocusableNode,
		forward: boolean = true
	): FocusableNode | undefined {
		if (!node) return;

		if (node.options.focusable) {
			return node;
		}

		// Only consider navigable children for navigation
		const navigableChildren = this.getNavigableChildNodes(node);
		const children = forward ? navigableChildren : navigableChildren.slice().reverse();

		const preferredChild = this.getPreferredChildNode(node);
		if (preferredChild) {
			const navigableChild = this.findNavigableDescendant(preferredChild, forward);
			if (navigableChild) {
				return navigableChild;
			}
		}

		for (const childNode of children) {
			// Container elements (those with vertical: true or navigable children) are not directly focusable
			if (this.isContainerElement(childNode)) {
				// Search within container elements but don't return them directly
				const subchild = this.findNavigableDescendant(childNode, forward);
				if (subchild) {
					return subchild;
				}
				continue;
			}
			if (childNode.options.focusable) {
				return childNode;
			}
		}

		return undefined;
	}

	// Finds next focusable within vertical column hierarchy
	//
	// Traverses up through vertical parents to find adjacent elements
	// while staying within the same column structure.
	private findNextInColumnNode(
		searchNode: FocusableNode,
		forward: boolean
	): FocusableNode | undefined {
		const parentNode = this.getParentNode(searchNode);
		if (!parentNode || !this.currentNode) return undefined;

		// Exit if parent is not vertical to stay within the same column
		if (!parentNode.options.vertical) {
			return undefined;
		}

		// Only consider navigable children for navigation (already as nodes)
		const navigableChildren = this.getNavigableChildNodes(parentNode);
		const currentIndex = navigableChildren.findIndex((n) => n.element === searchNode.element);

		if (currentIndex === -1) return undefined;

		// Get siblings to search in the specified direction
		const siblingsToSearch = forward
			? navigableChildren.slice(currentIndex + 1)
			: navigableChildren.slice(0, currentIndex).reverse();

		for (const nextChild of siblingsToSearch) {
			const result = this.findNavigableDescendant(nextChild, forward);
			if (result) return result;
		}

		// Recursively search parent if it's also vertical
		if (parentNode.options.vertical && parentNode.parent) {
			return this.findNextInColumnNode(parentNode, forward);
		}

		return undefined;
	}

	// Finds focusable in adjacent column for horizontal navigation
	findInNextColumn(element: HTMLElement, forward: boolean): FocusableNode | undefined {
		const node = this.getNode(element);
		if (!node) return undefined;

		let searchNode: FocusableNode | undefined = node.parent;
		while (searchNode) {
			// Find non-list parent container
			const ancestorNode = this.findHorizontalAncestorNode(searchNode);
			if (!ancestorNode) return;

			// Find the current branch from the non-list parent's perspective
			const ancestorChild = this.findChildNodeByDescendent(ancestorNode, element);
			if (!ancestorChild) return;

			// Find non-button siblings of the current branch for navigation
			const children = ancestorNode.children;
			const currentIndex = children.indexOf(ancestorChild);

			if (currentIndex !== -1) {
				// Get sibling nodes in the search direction
				const searchSiblings = forward
					? children.slice(currentIndex + 1)
					: children.slice(0, currentIndex).reverse();

				// Search through siblings for one that contains a vertical container
				for (const sibling of searchSiblings) {
					const childNode = this.findNavigableDescendant(sibling, forward);
					if (childNode) {
						return childNode;
					}
				}
			}
			searchNode = ancestorNode.parent;
		}
	}

	// Finds first non-vertical ancestor for cross-column navigation
	private findHorizontalAncestorNode(node: FocusableNode): FocusableNode | undefined {
		let current: FocusableNode | undefined = node;
		while (current) {
			if (!current.options.vertical) {
				return current;
			}
			current = current.parent;
		}
		return undefined;
	}

	// Finds which child branch contains the given element
	private findChildNodeByDescendent(
		ancestor: FocusableNode,
		element: HTMLElement
	): FocusableNode | undefined {
		// Check each child (including buttons) to see which one contains our current element
		// We need to check all children here because we're looking for containment, not navigation
		for (const child of ancestor.children) {
			if (this.isNodeDescendantOf(child, element)) {
				return child;
			}
		}

		return undefined;
	}

	// Checks if target element is a descendant of container node
	private isNodeDescendantOf(container: FocusableNode, target: HTMLElement): boolean {
		if (container.element === target) return true;

		// Recursively check children
		for (const child of container.children) {
			if (this.isNodeDescendantOf(child, target)) {
				return true;
			}
		}

		return false;
	}

	// ============================================
	// 8. METADATA & REGISTRY
	// ============================================

	private getNode(element: HTMLElement): FocusableNode | undefined {
		return this.nodeMap.get(element);
	}

	private isElementRegistered(element: HTMLElement): boolean {
		return this.nodeMap.has(element);
	}

	// ============================================
	// Node-based helpers to reduce lookups
	// ============================================

	// Gets parent node without redundant lookups
	private getParentNode(node: FocusableNode | undefined): FocusableNode | undefined {
		return node?.parent;
	}

	// Gets child nodes that can participate in navigation (non-buttons with content)
	private getNavigableChildNodes(node: FocusableNode): FocusableNode[] {
		return node.children.filter((child) => this.isNavigableNode(child));
	}

	// Checks if a node can participate in navigation
	private isNavigableNode(node: FocusableNode): boolean {
		return node.options.focusable || node.children.length > 0;
	}

	private establishParentChildRelationships(
		element: HTMLElement,
		newNode: FocusableNode,
		parentNode?: FocusableNode
	) {
		if (!parentNode) return;

		// Find children that should be moved to this new parent
		const childrenToMove = parentNode.children.filter(
			(child) => element.contains(child.element) && child.element !== element
		);

		// Move children to new parent
		for (const child of childrenToMove) {
			removeFromArray(parentNode.children, child);
			newNode.children.push(child);
			child.parent = newNode;
		}

		// Add this node to parent's children
		parentNode.children.push(newNode);
		// Sort children based on DOM order
		this.sortNodesByDomOrder(parentNode.children);
	}

	private resolvePendingRelationships() {
		for (const pending of this.pendingRelationships) {
			const pendingNode = this.nodeMap.get(pending);
			if (!pendingNode) continue;

			const parentNode = this.findParentNode(pending);
			if (parentNode) {
				pendingNode.parent = parentNode;
				parentNode.children.push(pendingNode);
				// Sort children by DOM order
				this.sortNodesByDomOrder(parentNode.children);
			}
		}
		this.pendingRelationships = this.pendingRelationships.filter((p) => {
			const node = this.nodeMap.get(p);
			return !node?.parent;
		});
	}

	private handlePendingRegistration(
		element: HTMLElement,
		parentNode: FocusableNode | undefined,
		options: FocusableOptions
	) {
		if (!parentNode && !options.isolate) {
			this.pendingRelationships.push(element);
		}
	}

	private unregisterElement(element: HTMLElement) {
		const node = this.nodeMap.get(element);
		if (!node) return;

		const parentNode = node.parent;

		// Remove this node from parent's children
		if (parentNode) {
			removeFromArray(parentNode.children, node);
		}

		// Move this node's children to its parent
		for (const child of node.children) {
			child.parent = parentNode;
			if (parentNode) {
				parentNode.children.push(child);
			}
		}

		// Sort parent's children if it exists
		if (parentNode) {
			this.sortNodesByDomOrder(parentNode.children);
		}

		this.nodeMap.delete(element);
	}

	// Finds the nearest registered parent node for an element
	private findParentNode(element: HTMLElement): FocusableNode | undefined {
		if (!element || !element.isConnected) return undefined;

		let current = element.parentElement;
		let depth = 0;
		// Prevent infinite loops
		while (current && depth < MAX_PARENT_SEARCH_DEPTH) {
			const focusable = this.nodeMap.get(current);
			if (focusable) {
				return focusable;
			}
			current = current.parentElement;
			depth++;
		}

		return undefined;
	}

	// ============================================
	// 9. ELEMENT CLASSIFICATION
	// ============================================

	// Checks if element is a container (has children or vertical flag)
	private isContainerElement(node: FocusableNode): boolean {
		const navigableChildren = this.getNavigableChildNodes(node);
		return node.options.vertical === true || navigableChildren.length > 0;
	}

	// ============================================
	// 10. CACHING & HISTORY
	// ============================================

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

	// Gets preferred child using cached index or first child
	private getPreferredChildNode(node: FocusableNode): FocusableNode | undefined {
		if (!node.children.length) {
			return undefined;
		}

		const cachedIndex = this.indexCache.get(node.element);
		if (cachedIndex !== undefined && cachedIndex < node.children.length) {
			const cachedChild = node.children[cachedIndex];
			// Check if cached child is still connected
			if (cachedChild && cachedChild.element.isConnected) {
				return cachedChild;
			}
		}

		// Return first child
		return node.children.at(0);
	}

	// Caches active indices for all parent containers
	private cacheActiveIndexNode(node: FocusableNode) {
		let currentNode = node;
		let parentNode = this.getParentNode(currentNode);

		// Traverse up the full hierarchy and cache indices for all parents
		while (parentNode) {
			// Cache index for all parents (not just vertical containers)
			const childIndex = parentNode.children.findIndex((c) => c.element === currentNode.element);
			if (childIndex >= 0) {
				this.indexCache.set(parentNode.element, childIndex);
			}

			// Move up to the next parent
			currentNode = parentNode;
			parentNode = this.getParentNode(currentNode);
		}
	}

	// ============================================
	// 11. NODE UTILITIES
	// ============================================

	// Sorts an array of nodes by their DOM position
	private sortNodesByDomOrder(nodes: FocusableNode[]): void {
		nodes.sort((a, b) => {
			if (a.element === b.element) return 0;
			const pos = a.element.compareDocumentPosition(b.element);
			return pos & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1;
		});
	}

	// ============================================
	// 12. CALLBACKS & EFFECTS
	// ============================================

	// Node-based callback triggering to avoid lookups
	private triggerOnActiveCallbacksNode(node: FocusableNode, active: boolean): void {
		let current: FocusableNode | undefined = node;
		while (current) {
			try {
				current.options.onActive?.(active);
				// If autoAction is enabled and element is becoming active, also trigger onAction
				if (active && current.options.autoAction && current.options.onAction) {
					current.options.onAction();
				}
			} catch (error) {
				console.warn(`Error in onActive(${active}) callback:`, error);
			}
			current = this.getParentNode(current);
		}
	}

	focusElement(element: HTMLElement): void {
		if (!element || !element.isConnected) return;

		if (element.tabIndex !== -1) {
			element.focus();
		} else {
			const activeElement = document.activeElement;
			if (activeElement instanceof HTMLElement && !element.contains(activeElement)) {
				activeElement.blur();
			}
		}
		scrollIntoViewIfNeeded(element);
	}

	// ============================================
	// 13. UTILITIES
	// ============================================

	private tryEscapeHandlers(event: KeyboardEvent): boolean {
		if (!this.currentNode) return false;

		const escHandlers = this.getNodeEscHandlers(this.currentNode);

		for (const escHandler of escHandlers) {
			try {
				const handled = escHandler();
				if (handled === true) {
					event.preventDefault();
					event.stopPropagation();
					return true;
				}
			} catch (error) {
				console.warn('Error in onEsc handler:', error);
			}
		}

		return false;
	}

	private getNodeEscHandlers(node: FocusableNode): (() => boolean | void)[] {
		const handlers: (() => boolean | void)[] = [];
		let current: FocusableNode | undefined = node;

		while (current) {
			if (current.options.onEsc) {
				handlers.push(current.options.onEsc as () => boolean | void);
			}
			current = current.parent;
		}
		return handlers;
	}

	private getNodeKeydownHandlers(node: FocusableNode): KeyboardHandler[] {
		const handlers: KeyboardHandler[] = [];
		let current: FocusableNode | undefined = node;

		while (current) {
			if (current.options.onKeydown) {
				handlers.push(current.options.onKeydown);
			}
			current = current.parent;
		}
		return handlers;
	}

	private buildNavigationContext(event: KeyboardEvent, node: FocusableNode): NavigationContext {
		const { key, metaKey, ctrlKey, shiftKey } = event;

		return {
			action: getNavigationAction(key),
			trap: node.options.trap,
			shiftKey,
			ctrlKey,
			metaKey
		};
	}

	private shouldShowOutlineOnly(context: { action: NavigationAction | null }): boolean {
		return !this._outline && context.action !== null;
	}

	getOptions(element?: HTMLElement): FocusableOptions | null {
		return element ? this.nodeMap.get(element)?.options || null : null;
	}

	private printTreeNode(element: HTMLElement, logElements: boolean, depth: number): void {
		const node = this.getNode(element);
		if (!node) return;

		// Build indentation
		const indent = '  '.repeat(depth);

		// Build node description
		const description = getElementDescription(element);
		const isCurrent = this.currentNode?.element === element;
		const marker = isCurrent ? ' ◀── CURRENT' : '';

		// Build status indicators
		const flags: string[] = [];
		if (node.options.disabled) flags.push('disabled');
		if (node.options.trap) flags.push('trap');
		if (node.options.vertical) flags.push('vertical');
		if (node.options.isolate) flags.push('isolate');
		if (this.isContainerElement(node)) flags.push('container');
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

		for (const child of node.children) {
			this.printTreeNode(child.element, logElements, depth + 1);
		}
	}
}
