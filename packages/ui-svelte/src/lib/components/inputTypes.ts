/**
 * Shared types for input components (Textbox, Textarea, TagInput)
 */

export interface BaseInputProps {
	id?: string;
	testId?: string;
	label?: string;
	placeholder?: string;
	disabled?: boolean;
	readonly?: boolean;
	autofocus?: boolean;
	error?: string;
	helperText?: string;
}

export interface InputStylingProps {
	wide?: boolean;
	width?: number;
}

export interface InputStateCallbacks {
	oninput?: (val: string) => void;
	onchange?: (val: string) => void;
	onblur?: (e: FocusEvent) => void;
	onfocus?: (e: FocusEvent) => void;
}
