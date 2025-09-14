import { findNearestSuitableAncestor } from '$lib/focus/domUtils';

// F key is reserved for F-mode toggle, so 'FF' is not a valid shortcut
const RESERVED_SHORTCUT = 'FF';

function isValidShortcut(shortcut: string, used: Map<string, unknown>): boolean {
	return shortcut !== RESERVED_SHORTCUT && !used.has(shortcut);
}

// Prioritizes left hand and home row keys for ergonomics
export function generateTwoLetterShortcut(used: Map<string, unknown>): string | undefined {
	const keyGroups = [
		// Group 1: Left hand home row (highest priority)
		['A', 'S', 'D'],
		// Group 2: Extended left hand home row including G
		['A', 'S', 'D', 'G'],
		// Group 3: Left hand upper row
		['Q', 'W', 'E', 'R', 'T'],
		// Group 4: Left hand lower row
		['Z', 'X', 'C', 'V', 'B'],
		// Group 5: Right hand home row (for second letter when needed)
		['J', 'K', 'L'],
		// Group 6: All other letters as fallback
		['H', 'I', 'M', 'N', 'O', 'P', 'U', 'Y']
	];

	for (let groupIndex = 0; groupIndex < 3; groupIndex++) {
		const group = keyGroups[groupIndex];
		for (const first of group) {
			for (const second of group) {
				const shortcut = first + second;
				if (isValidShortcut(shortcut, used)) {
					return shortcut;
				}
			}
		}
	}

	const leftHandKeys = [...keyGroups[0], ...keyGroups[1], ...keyGroups[2], ...keyGroups[3]];
	const uniqueLeftKeys = [...new Set(leftHandKeys)]; // Remove duplicates

	for (const first of uniqueLeftKeys) {
		for (const second of uniqueLeftKeys) {
			const shortcut = first + second;
			if (isValidShortcut(shortcut, used)) {
				return shortcut;
			}
		}
	}

	const allPreferredKeys = [...uniqueLeftKeys, ...keyGroups[4]];
	for (const first of allPreferredKeys) {
		for (const second of allPreferredKeys) {
			const shortcut = first + second;
			if (isValidShortcut(shortcut, used)) {
				return shortcut;
			}
		}
	}

	for (let first = 65; first <= 90; first++) {
		for (let second = 65; second <= 90; second++) {
			const shortcut = String.fromCharCode(first) + String.fromCharCode(second);
			if (isValidShortcut(shortcut, used)) {
				return shortcut;
			}
		}
	}

	return undefined;
}

// Avoids overflow issues with transformed or fixed position containers
export function createShortcutOverlay(element: HTMLElement, shortcut: string): HTMLElement {
	const { ancestor, accumulatedLeft, accumulatedTop } = findNearestSuitableAncestor(element);

	const overlay = document.createElement('div');
	overlay.className = 'focus-shortcut-overlay';
	overlay.textContent = shortcut;

	overlay.style.position = 'absolute';
	overlay.style.left = `${accumulatedLeft + element.offsetWidth - 12}px`; // Position at right edge
	overlay.style.top = `${accumulatedTop + element.offsetHeight - 6}px`; // Position at bottom edge
	overlay.style.zIndex = '99999';
	overlay.style.padding = '2px 4px';
	overlay.style.fontSize = '10px';
	overlay.style.fontWeight = '600';
	overlay.style.borderRadius = '4px';
	overlay.style.border = '1px solid var(--clr-border-1)';
	overlay.style.backgroundColor = 'var(--clr-bg-1)';
	overlay.style.color = 'var(--clr-text-1)';
	overlay.style.pointerEvents = 'none';
	overlay.style.userSelect = 'none';

	ancestor.appendChild(overlay);

	return overlay;
}

export function removeShortcutOverlay(overlay: HTMLElement): void {
	overlay.remove();
}
