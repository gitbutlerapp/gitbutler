<script lang="ts">
	import { Textbox } from "@gitbutler/ui";

	interface Props {
		// Username-specific props
		customValidationMessage?: string;
		minLength?: number;
		maxLength?: number;
		value?: string;
		label?: string;
		placeholder?: string;
		// All other props are forwarded to Textbox
		[key: string]: any;
	}

	let {
		customValidationMessage = "Bitte gib einen gültigen Benutzernamen ein.",
		minLength = 3,
		maxLength = 30,
		value = $bindable(),
		label = "Benutzername",
		...restProps
	}: Props = $props();

	let usernameError = $state<string | undefined>(undefined);
	let usernameTouched = $state(false);

	function validateUsername(val: string): { isValid: boolean; message?: string } {
		if (!val) return { isValid: true }; // Empty is valid (unless required)

		// Check length
		if (val.length < minLength) {
			return {
				isValid: false,
				message: `Der Benutzername muss mindestens ${minLength} Zeichen lang sein.`,
			};
		}

		if (val.length > maxLength) {
			return {
				isValid: false,
				message: `Der Benutzername darf höchstens ${maxLength} Zeichen lang sein.`,
			};
		}

		// Check for valid characters: alphanumeric, underscores, hyphens
		// Must start with alphanumeric character
		if (!/^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(val)) {
			return {
				isValid: false,
				message:
					"Der Benutzername muss mit einem Buchstaben oder einer Zahl beginnen und darf nur Buchstaben, Zahlen, Unterstriche und Bindestriche enthalten.",
			};
		}

		// Cannot end with hyphen or underscore
		if (/[-_]$/.test(val)) {
			return {
				isValid: false,
				message: "Der Benutzername darf nicht mit einem Bindestrich oder Unterstrich enden.",
			};
		}

		// Cannot have consecutive special characters
		if (/[-_]{2,}/.test(val)) {
			return {
				isValid: false,
				message: "Der Benutzername darf keine aufeinanderfolgenden Bindestriche oder Unterstriche enthalten.",
			};
		}

		return { isValid: true };
	}

	function handleInput(val: string) {
		value = val;

		// Only show validation errors after the field has been touched (blurred once)
		if (usernameTouched) {
			const validation = validateUsername(val);
			usernameError =
				val && !validation.isValid ? validation.message || customValidationMessage : undefined;
		}
	}

	function handleChange() {
		// Mark as touched when user leaves the field
		usernameTouched = true;

		// Validate on blur
		if (value) {
			const validation = validateUsername(value);
			if (!validation.isValid) {
				usernameError = validation.message || customValidationMessage;
			} else {
				usernameError = undefined;
			}
		} else {
			usernameError = undefined;
		}
	}

	// Export validation state for parent components
	export function isValid(): boolean {
		if (!value) return true;
		return validateUsername(value).isValid;
	}

	export function validate(): boolean {
		usernameTouched = true;
		if (value) {
			const validation = validateUsername(value);
			if (!validation.isValid) {
				usernameError = validation.message || customValidationMessage;
				return false;
			} else {
				usernameError = undefined;
				return true;
			}
		} else {
			usernameError = undefined;
			return true;
		}
	}
</script>

<Textbox
	{...restProps}
	{label}
	type="text"
	bind:value
	error={usernameError}
	oninput={handleInput}
	onchange={handleChange}
/>
