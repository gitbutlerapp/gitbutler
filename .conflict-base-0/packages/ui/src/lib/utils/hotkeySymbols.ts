import { determinePlatform } from '$lib/utils/platform';

// Symbol-to-key mapping for displaying and parsing hotkeys
export const SYMBOL_TO_KEY_MAP: Record<string, string> = {
	// Modifiers
	'⌘': 'Meta',
	'⌃': 'Control',
	'⌥': 'Alt',
	'⇧': 'Shift',
	cmd: 'Meta',
	ctrl: 'Control',
	alt: 'Alt',
	shift: 'Shift',

	// Special keys
	'↵': 'Enter',
	'⏎': 'Enter',
	'⎋': 'Escape',
	'⌫': 'Backspace',
	'⌦': 'Delete',
	'⇥': 'Tab',
	'␣': ' ',
	'⏺': ' ',
	'↑': 'ArrowUp',
	'↓': 'ArrowDown',
	'←': 'ArrowLeft',
	'→': 'ArrowRight',
	'⇞': 'PageUp',
	'⇟': 'PageDown',
	'↖': 'Home',
	'↘': 'End',

	// Function keys
	F1: 'F1',
	F2: 'F2',
	F3: 'F3',
	F4: 'F4',
	F5: 'F5',
	F6: 'F6',
	F7: 'F7',
	F8: 'F8',
	F9: 'F9',
	F10: 'F10',
	F11: 'F11',
	F12: 'F12'
};

// Platform-specific symbol mappings
export const PLATFORM_SYMBOLS = {
	macos: {
		Meta: '⌘',
		Control: '⌃',
		Alt: '⌥',
		Shift: '⇧'
	},
	windows: {
		Meta: 'Win',
		Control: 'Ctrl',
		Alt: 'Alt',
		Shift: 'Shift'
	},
	linux: {
		Meta: 'Super',
		Control: 'Ctrl',
		Alt: 'Alt',
		Shift: 'Shift'
	}
} as const;

// Reverse mapping for display purposes (Mac symbols by default)
export const KEY_TO_SYMBOL_MAP: Record<string, string> = {
	Meta: '⌘',
	Control: '⌃',
	Alt: '⌥',
	Shift: '⇧',
	Enter: '↵',
	Escape: '⎋',
	Backspace: '⌫',
	Delete: '⌦',
	Tab: '⇥',
	' ': '␣',
	ArrowUp: '↑',
	ArrowDown: '↓',
	ArrowLeft: '←',
	ArrowRight: '→',
	PageUp: '⇞',
	PageDown: '⇟',
	Home: '↖',
	End: '↘'
};

export interface ParsedHotkey {
	modifiers: {
		meta?: boolean;
		ctrl?: boolean;
		alt?: boolean;
		shift?: boolean;
	};
	key: string;
}

/**
 * Parse a hotkey string with symbols into modifier flags and key
 * Format: {modifier}{key} e.g., "⌘K", "⌃⇧S"
 */
export function parseHotkey(hotkey: string): ParsedHotkey | undefined {
	if (!hotkey) return undefined;

	const modifiers = {
		meta: false,
		ctrl: false,
		alt: false,
		shift: false
	};

	let remaining = hotkey;
	let foundModifier = true;

	// Extract modifiers
	while (foundModifier && remaining.length > 0) {
		foundModifier = false;

		// Check for modifier symbols at the start
		for (const [symbol, key] of Object.entries(SYMBOL_TO_KEY_MAP)) {
			if (remaining.startsWith(symbol)) {
				switch (key) {
					case 'Meta':
						modifiers.meta = true;
						break;
					case 'Control':
						modifiers.ctrl = true;
						break;
					case 'Alt':
						modifiers.alt = true;
						break;
					case 'Shift':
						modifiers.shift = true;
						break;
					default:
						continue;
				}
				remaining = remaining.slice(symbol.length);
				foundModifier = true;
				break;
			}
		}
	}

	// The remaining part is the key
	if (!remaining) return undefined;

	// Check if remaining is a symbol that needs conversion
	const mappedKey = SYMBOL_TO_KEY_MAP[remaining] || remaining;

	return {
		modifiers,
		key: mappedKey
	};
}

/**
 * Check if a keyboard event matches a parsed hotkey
 */
export function matchesHotkey(event: KeyboardEvent, parsed: ParsedHotkey): boolean {
	// Check for exact modifier match - both required and not required
	if (parsed.modifiers.meta !== event.metaKey) return false;
	if (parsed.modifiers.ctrl !== event.ctrlKey) return false;
	if (parsed.modifiers.alt !== event.altKey) return false;
	if (parsed.modifiers.shift !== event.shiftKey) return false;

	// Check if the key matches (case-insensitive for letters)
	const eventKey = event.key.length === 1 ? event.key.toUpperCase() : event.key;
	const hotkeyKey = parsed.key.length === 1 ? parsed.key.toUpperCase() : parsed.key;

	return eventKey === hotkeyKey;
}

/**
 * Convert a hotkey to display format with symbols
 * @param hotkey - The hotkey string to format
 */
export function formatHotkeyForDisplay(hotkey: string): string {
	const parsed = parseHotkey(hotkey);
	if (!parsed) return hotkey;

	let display = '';

	if (parsed.modifiers.meta) display += KEY_TO_SYMBOL_MAP['Meta'] || '⌘';
	if (parsed.modifiers.ctrl) display += KEY_TO_SYMBOL_MAP['Control'] || '⌃';
	if (parsed.modifiers.alt) display += KEY_TO_SYMBOL_MAP['Alt'] || '⌥';
	if (parsed.modifiers.shift) display += KEY_TO_SYMBOL_MAP['Shift'] || '⇧';

	// Convert key to symbol if available
	const keySymbol = KEY_TO_SYMBOL_MAP[parsed.key] || parsed.key;
	display += keySymbol;

	return display;
}

/**
 * Convert a hotkey to platform-aware display format
 * Mac hotkeys are converted to appropriate Windows/Linux representations
 * @param hotkey - The hotkey string in Mac format (e.g., "⌘S" or "cmd+S")
 */
export function formatHotkeyForPlatform(hotkey: string): string {
	const parsed = parseHotkey(hotkey);
	if (!parsed) return hotkey;

	// Determine platform
	const platform = determinePlatform(navigator.userAgent);
	const symbols =
		PLATFORM_SYMBOLS[platform as keyof typeof PLATFORM_SYMBOLS] || PLATFORM_SYMBOLS.macos;

	let display = '';

	// Build modifier string based on platform
	if (parsed.modifiers.meta) {
		// On Windows/Linux, Cmd (Meta) typically maps to Ctrl
		if (platform === 'windows' || platform === 'linux') {
			display += symbols['Control'];
		} else {
			display += symbols['Meta'];
		}
	}
	if (parsed.modifiers.ctrl) display += symbols['Control'];
	if (parsed.modifiers.alt) display += symbols['Alt'];
	if (parsed.modifiers.shift) display += symbols['Shift'];

	// Add separator for text-based platforms
	if ((platform === 'windows' || platform === 'linux') && display) {
		display += '+';
	}

	// Convert key to symbol if available (for special keys like arrows)
	const keySymbol = (platform === 'macos' && KEY_TO_SYMBOL_MAP[parsed.key]) || parsed.key;
	display += keySymbol;

	return display;
}
