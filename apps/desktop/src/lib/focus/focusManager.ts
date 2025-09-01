import { focusNextTabIndex } from '$lib/focus/tabbable';
import {
	addToArray,
	removeFromArray,
	scrollIntoViewIfNeeded,
	sortByDomOrder
} from '$lib/focus/utils';
import { InjectionToken } from '@gitbutler/shared/context';
import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
import { on } from 'svelte/events';
import { writable } from 'svelte/store';

export const FOCUS_MANAGER = new InjectionToken<FocusManager>('FocusManager');

export enum DefinedFocusable {
	MainViewport = 'viewport',
	ViewportLeft = 'viewport-left',
	ViewportRight = 'viewport-right',
	ViewportDrawerRight = 'viewport-drawer-right',
	ViewportMiddle = 'viewport-middle',
	UncommittedChanges = 'uncommitted-changes',
	Drawer = 'drawer',
	Branches = 'branches',
	Branch = 'branch',
	Stack = 'stack',
	Preview = 'preview',
	ChangedFiles = 'changed-files',
	Commit = 'commit',
	CommitList = 'commit-list',
	FileItem = 'file-item',
	FileList = 'file-list',
	ChromeSidebar = 'chrome-sidebar',
	ChromeHeader = 'chrome-header',
	Chrome = 'chrome',
	Rules = 'rules'
}

export type Payload = Record<string, unknown>;

type NavigationAction = 'tab' | 'prev' | 'next' | 'exit' | 'enter';

// Common payload types for type safety
export interface StackPayload {
	stackId: string;
	branchName?: string;
	isActive?: boolean;
}

export interface ItemPayload {
	itemId: string;
	index: number;
	data?: any;
}

export interface FilePayload {
	filePath: string;
	isModified?: boolean;
	lineCount?: number;
}

export interface BranchPayload {
	branchName: string;
	stackId?: string;
	isActive?: boolean;
	commitId?: string;
}

export interface CommitPayload {
	commitId: string;
	stackId?: string;
	branchName?: string;
	message?: string;
}

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
	id?: DefinedFocusable;
	tabIndex?: number; // Custom tab order within siblings
	disabled?: boolean; // Skip this element during navigation
	payload?: T;
	skip?: boolean;
	list?: boolean;
	default?: boolean;
	onKeydown?: KeyboardHandler<T>;
	onFocus?: (context: FocusContext<T>) => void;
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

	readonly cursor = writable<HTMLElement | undefined>();
	readonly outline = writable(false);

	private handleMouse = this.handleClick.bind(this);
	private handleKeys = this.handleKeydown.bind(this);

	constructor() {}

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

		const parentElement = this.findParent(element);
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

		if (!parentMeta) {
			this.pendingRelationships.push(element);
		}

		if (options.default) {
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

	unregister(logicalId?: DefinedFocusable, element?: HTMLElement) {
		if (element) {
			// Unregister specific element
			this.unregisterElement(element);
		}

		// Remove pending relationships
		this.pendingRelationships = this.pendingRelationships.filter((p) => p !== element);

		// Clear current if it matches
		if (this._currentElement) {
			const currentMeta = this.getMetadata(this._currentElement);
			if (!currentMeta || (element && this._currentElement === element)) {
				this._currentElement = undefined;
				// Hide cursor when current element is unregistered
				this.cursor.set(undefined);
			}
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
		let element = this.getCurrentMetadata()?.parentElement;
		if (element) {
			let parentData = this.getMetadata(element);
			while (parentData?.options.skip) {
				element = parentData.parentElement;
				parentData = element ? this.getMetadata(element) : undefined;
			}
			if (element) {
				this.activateAndFocus(element);
			}
		}
	}

	focusFirstChild(): boolean {
		const metadata = this.getCurrentMetadata();
		let firstChild = metadata?.children.at(0);
		let childData = firstChild ? this.getMetadataOrThrow(firstChild) : undefined;

		while (childData?.options.skip) {
			firstChild = childData.children.at(0);
			childData = firstChild ? this.getMetadataOrThrow(firstChild) : undefined;
		}
		return firstChild ? this.activateAndFocus(firstChild) : false;
	}

	handleClick(e: Event) {
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
		// Ignore keyboard navigation if user is focused on an
		// input element, or no current element.
		if (
			(event.target instanceof HTMLElement && event.target.contentEditable) ||
			event.target instanceof HTMLTextAreaElement ||
			event.target instanceof HTMLInputElement ||
			!this._currentElement
		) {
			return;
		}

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

		event.preventDefault();
		event.stopPropagation();

		this.executeNavigationAction(navigationAction, {
			metaKey: event.metaKey,
			forward: !event.shiftKey,
			list
		});
		return true;
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
		}
	): void {
		const { metaKey, list: isList, forward = true } = options;
		switch (action) {
			case 'tab':
				if (this._currentElement) {
					focusNextTabIndex({ container: this._currentElement, forward });
				}
				break;
			case 'prev':
				if (metaKey && !isList) {
					this.focusCousin(false);
				} else if (!this.focusSibling(false)) {
					this.focusParent();
				}
				break;

			case 'next':
				if (metaKey && !isList) {
					this.focusCousin(true);
				} else if (!this.focusSibling(true)) {
					this.focusFirstChild();
				}
				break;

			case 'exit':
				if (metaKey && isList) {
					this.focusCousin(false);
				} else {
					this.focusParent();
				}
				break;

			case 'enter':
				if (metaKey && isList) {
					this.focusCousin(true);
				} else if (!this.focusFirstChild()) {
					this.focusSibling(true);
				}
				break;
		}
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
