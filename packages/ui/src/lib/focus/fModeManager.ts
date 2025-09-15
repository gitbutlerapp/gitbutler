import {
	generateTwoLetterShortcut,
	createShortcutOverlay,
	removeShortcutOverlay
} from '$lib/focus/shortcutUtils';
import type { FocusableNode } from '$lib/focus/focusTypes';

/**
 * F-mode allows users to quickly navigate to buttons and containers using
 * two-letter keyboard shortcuts (e.g., 'AA', 'AB', etc.).
 */
export class FModeManager {
	private _active = false;
	private firstLetter: string | undefined;
	private shortcuts = new Map<string, HTMLElement>();
	private featureEnabled = false;

	private activeOverlays = new Map<HTMLElement, HTMLElement>();
	private elementToShortcut = new Map<HTMLElement, string>();

	get active(): boolean {
		return this._active;
	}

	setFeatureEnabled(enabled: boolean): void {
		this.featureEnabled = enabled;
		if (!enabled && this._active) {
			this.deactivate();
		}
	}

	handleKeypress(
		event: KeyboardEvent,
		elements?: Map<HTMLElement, FocusableNode>,
		currentNode?: FocusableNode
	): boolean {
		if (!this.featureEnabled) return false;

		const key = event.key;

		if (key === 'f' && !this._active) {
			this.activate(elements, currentNode);
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		if (this._active) {
			if (key === 'Escape') {
				// If first letter was typed, reset to show all shortcuts
				if (this.firstLetter) {
					this.firstLetter = undefined;
					this.showAllShortcuts();
				} else {
					// Otherwise deactivate F mode
					this.deactivate();
				}
				event.preventDefault();
				event.stopPropagation();
				return true;
			}

			if (key === 'f') {
				this.deactivate();
				event.preventDefault();
				event.stopPropagation();
				return true;
			}

			const upperKey = key.toUpperCase();

			if (upperKey.length !== 1 || upperKey < 'A' || upperKey > 'Z') {
				return false;
			}

			event.preventDefault();
			event.stopPropagation();

			if (!this.firstLetter) {
				// First letter typed - filter shortcuts
				this.firstLetter = upperKey;
				const hasMatchingShortcuts = this.filterShortcuts(upperKey);

				// If no shortcuts start with this letter, dismiss
				if (!hasMatchingShortcuts) {
					this.deactivate();
				}
			} else {
				// Second letter typed - check for exact match
				const shortcut = this.firstLetter + upperKey;
				const element = this.shortcuts.get(shortcut);

				if (element) {
					this.handleShortcutActivation(element);
				} else {
					// No matching shortcut - dismiss instead of continuing
					this.deactivate();
				}
			}

			return true;
		}

		return false;
	}

	activate(elements?: Map<HTMLElement, FocusableNode>, currentNode?: FocusableNode): void {
		if (this._active) return;

		this._active = true;
		this.firstLetter = undefined;
		this.shortcuts.clear();

		if (elements) {
			// Check if current element has trap: true
			const trapElement = currentNode?.options.trap ? currentNode.element : undefined;

			for (const [element, node] of elements) {
				// If we have a trap element, only add elements contained within it
				if (trapElement) {
					if (trapElement.contains(element)) {
						this.addElement(element, node);
					}
				} else {
					this.addElement(element, node);
				}
			}
		}
	}

	deactivate(): void {
		if (!this._active) return;

		this._active = false;
		this.firstLetter = undefined;
		this.hideAllShortcuts();
		this.shortcuts.clear();
		this.elementToShortcut.clear();
	}

	addElement(element: HTMLElement, node: FocusableNode): string | undefined {
		if (!this._active) return undefined;

		if (!node.options.button) return undefined;

		const shortcut = generateTwoLetterShortcut(this.shortcuts);
		if (!shortcut) return undefined;

		this.shortcuts.set(shortcut, element);
		this.elementToShortcut.set(element, shortcut);
		this.showShortcut(element, shortcut);

		return shortcut;
	}

	removeElement(element: HTMLElement): void {
		const shortcutToRemove = this.elementToShortcut.get(element);
		if (!shortcutToRemove) return;

		this.shortcuts.delete(shortcutToRemove);
		this.elementToShortcut.delete(element);
		this.hideShortcut(element);
	}

	clear(): void {
		this.hideAllShortcuts();
		this.shortcuts.clear();
		this.elementToShortcut.clear();
		this.firstLetter = undefined;
	}

	private handleShortcutActivation(element: HTMLElement): void {
		if (!element.isConnected) {
			this.removeElement(element);
			return;
		}
		element.click();
		this.deactivate(); // Auto-exit after activation
	}

	private showShortcut(element: HTMLElement, shortcut: string): void {
		this.hideShortcut(element);
		const overlay = createShortcutOverlay(element, shortcut);
		this.activeOverlays.set(element, overlay);
		element.classList.add('shortcut-visible');
	}

	private hideShortcut(element: HTMLElement): void {
		const overlay = this.activeOverlays.get(element);
		if (overlay) {
			removeShortcutOverlay(overlay);
			this.activeOverlays.delete(element);
		}
		element.classList.remove('shortcut-visible');
	}

	private hideAllShortcuts(): void {
		Array.from(this.activeOverlays.entries()).forEach(([element, overlay]) => {
			removeShortcutOverlay(overlay);
			element.removeAttribute('data-fmode-button');
			element.classList.remove('shortcut-visible');
		});
		this.activeOverlays.clear();
	}

	// For debugging
	getShortcuts(): ReadonlyMap<string, HTMLElement> {
		return this.shortcuts;
	}

	private filterShortcuts(prefix: string): boolean {
		let hasMatches = false;

		Array.from(this.shortcuts.entries()).forEach(([shortcut, element]) => {
			if (shortcut.startsWith(prefix)) {
				// Keep this shortcut visible
				hasMatches = true;
			} else {
				// Hide shortcuts that don't match the prefix
				this.hideShortcut(element);
			}
		});

		return hasMatches;
	}

	private showAllShortcuts(): void {
		Array.from(this.shortcuts.entries()).forEach(([shortcut, element]) => {
			if (!this.activeOverlays.has(element)) {
				this.showShortcut(element, shortcut);
			}
		});
	}
}
