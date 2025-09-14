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

	private activeOverlays = new Map<HTMLElement, HTMLElement>();

	get active(): boolean {
		return this._active;
	}

	handleKeypress(event: KeyboardEvent, elements?: Map<HTMLElement, FocusableNode>): boolean {
		const key = event.key;

		if (key === 'f' && !this._active) {
			this.activate(elements);
			event.preventDefault();
			event.stopPropagation();
			return true;
		}

		if (this._active) {
			if (key === 'Escape' || key === 'f') {
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
				this.firstLetter = upperKey;
			} else {
				const shortcut = this.firstLetter + upperKey;
				const element = this.shortcuts.get(shortcut);

				if (element) {
					this.handleShortcutActivation(element);
				}

				this.firstLetter = undefined;
			}

			return true;
		}

		return false;
	}

	activate(elements?: Map<HTMLElement, FocusableNode>): void {
		if (this._active) return;

		this._active = true;
		this.firstLetter = undefined;
		this.shortcuts.clear();

		if (elements) {
			for (const [element, node] of elements) {
				this.addElement(element, node);
			}
		}
	}

	deactivate(): void {
		if (!this._active) return;

		this._active = false;
		this.firstLetter = undefined;
		this.hideAllShortcuts();
		this.shortcuts.clear();
	}

	addElement(element: HTMLElement, node: FocusableNode): string | undefined {
		if (!this._active) return undefined;

		if (!node.options.button) return undefined;

		const shortcut = generateTwoLetterShortcut(this.shortcuts);
		if (!shortcut) return undefined;

		this.shortcuts.set(shortcut, element);
		this.showShortcut(element, shortcut);

		return shortcut;
	}

	removeElement(element: HTMLElement): void {
		let shortcutToRemove: string | undefined;
		Array.from(this.shortcuts.entries()).forEach(([shortcut, el]) => {
			if (el === element && !shortcutToRemove) {
				shortcutToRemove = shortcut;
			}
		});

		if (!shortcutToRemove) return;

		this.shortcuts.delete(shortcutToRemove);
		this.hideShortcut(element);
	}

	clear(): void {
		this.hideAllShortcuts();
		this.shortcuts.clear();
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
}
