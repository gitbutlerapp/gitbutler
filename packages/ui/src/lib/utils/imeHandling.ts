/**
 * IME (Input Method Editor) handling utilities for text input components.
 * This class provides a unified handler to manage IME composition state
 * and prevent unintended keyboard shortcuts during text composition in
 * Japanese, Chinese, Korean, and other languages that require input method editors.
 */
export class IMECompositionHandler {
	private _isComposing = false;

	get isComposing(): boolean {
		return this._isComposing;
	}

	setComposing(composing: boolean): void {
		this._isComposing = composing;
	}

	reset(): void {
		this._isComposing = false;
	}

	/**
	 * Creates an input event handler that tracks IME composition state
	 *
	 * @param originalHandler - Optional original input handler to call
	 * @returns Input event handler function
	 */
	handleInput(originalHandler?: (e: Event) => void) {
		return (event: Event) => {
			if (event instanceof InputEvent) {
				this.setComposing(event.isComposing);
			}

			originalHandler?.(event);
		};
	}

	/**
	 * Creates a keydown event handler that blocks actions during IME composition
	 *
	 * @param originalHandler - Optional original keydown handler to call
	 * @param additionalBlockingKeys - Additional keys to block during composition
	 * @returns Keydown event handler function
	 */
	handleKeydown(
		originalHandler?: (event: KeyboardEvent) => void,
		additionalBlockingKeys: string[] = []
	) {
		return (event: KeyboardEvent) => {
			if (
				[...IME_BLOCKING_KEYS, ...additionalBlockingKeys].includes(event.key) &&
				this.isComposing
			) {
				event.preventDefault();
				event.stopPropagation();
				this.reset();
				return;
			}

			originalHandler?.(event);
		};
	}
}

/**
 * Keys that should be blocked during IME composition to prevent unintended actions
 */
const IME_BLOCKING_KEYS = [
	'Enter',
	'Escape',
	'Tab',
	'0',
	'1',
	'2',
	'3',
	'4',
	'5',
	'6',
	'7',
	'8',
	'9'
] as const;
