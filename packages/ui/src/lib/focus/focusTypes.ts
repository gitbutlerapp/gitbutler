type NavigationAction = 'tab' | 'left' | 'right' | 'up' | 'down';
export type { NavigationAction };

export type KeyboardHandler = (event: KeyboardEvent) => boolean | void;

export type NavigationContext = {
	action: NavigationAction | null;
	trap?: boolean;
	inVertical?: boolean;
	isInput?: boolean;
	// User selected text detected.
	hasSelection?: boolean;
	hasOutline?: boolean;
	shiftKey: boolean;
	ctrlKey: boolean;
	metaKey: boolean;
};

export interface FocusableOptions {
	// Custom tab order within siblings (overrides default DOM order)
	tabIndex?: number;
	// Keep focusable inactive and outside navigation hierarchy
	disabled?: boolean;
	// Treat children as vertically oriented (changes arrow key behavior, automatically skips during navigation)
	vertical?: boolean;
	// Automatically focus this element when registered
	activate?: boolean;
	// Prevent keyboard navigation from leaving this element
	trap?: boolean;
	// Never highlight the element
	dim?: boolean;
	// Automatically trigger onAction when this element becomes active
	autoAction?: boolean;
	// Register as a button (excluded from keyboard navigation but accessible via F mode)
	button?: boolean;
	// Hotkey combination for instant activation (e.g., "⌘K", "⇧⌘P", "⌃⌥A")
	hotkey?: string;
	// Whether this element can receive focus (default: true)
	focusable?: boolean;

	// Custom keyboard event handler, can prevent default navigation
	onKeydown?: KeyboardHandler;
	// Called when this element becomes the active focus or loses it
	onActive?: (value: boolean) => void;
	// Called when Space or Enter is pressed on this focused element, or when autoAction is true and element becomes active
	onAction?: () => boolean | void;
	// Called when Escape is pressed on this focused element
	onEsc?: () => boolean;
}

export interface FocusableNode {
	element: HTMLElement;
	parent?: FocusableNode; // Direct node reference for efficient traversal
	children: FocusableNode[]; // Direct node references for efficient traversal
	options: FocusableOptions;
}
