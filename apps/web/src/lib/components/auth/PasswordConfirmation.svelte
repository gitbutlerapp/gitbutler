<script lang="ts">
	import { Textbox } from "@gitbutler/ui";

	interface Props {
		password?: string;
		passwordConfirmation?: string;
		showValidation?: boolean;
		autocomplete?: boolean;
	}

	let {
		password = $bindable(),
		passwordConfirmation = $bindable(),
		showValidation = true,
		autocomplete = true,
	}: Props = $props();

	let passwordTouched = $state(false);
	let passwordConfirmationTouched = $state(false);

	const passwordsMatch = $derived(password === passwordConfirmation);

	function validatePassword(pwd: string) {
		if (!pwd) return { isValid: false, errors: [] };

		const errors = [];

		// Length check (minimum 8 characters)
		if (pwd.length < 8) {
			errors.push("mindestens 8 Zeichen");
		}

		// Must contain at least one lowercase letter
		if (!/[a-z]/.test(pwd)) {
			errors.push("einen Kleinbuchstaben");
		}

		// Must contain at least one uppercase letter
		if (!/[A-Z]/.test(pwd)) {
			errors.push("einen Großbuchstaben");
		}

		// Must contain at least one number
		if (!/\d/.test(pwd)) {
			errors.push("eine Zahl");
		}

		return { isValid: errors.length === 0, errors };
	}

	const passwordValidation = $derived(validatePassword(password || ""));
	const isPasswordValid = $derived(passwordValidation.isValid);

	const passwordError = $derived(
		showValidation && passwordTouched && password && !isPasswordValid
			? `Das Passwort muss enthalten: ${passwordValidation.errors.join(", ")}`
			: undefined,
	);

	const passwordHelperText = $derived(
		showValidation && password && isPasswordValid
			? "Starkes Passwort! ✅"
			: showValidation
				? "Mindestens 8 Zeichen mit Groß-, Kleinbuchstaben und einer Zahl"
				: undefined,
	);

	const passwordConfirmationError = $derived(
		passwordConfirmationTouched && passwordConfirmation && !passwordsMatch
			? "Passwörter stimmen nicht überein"
			: undefined,
	);

	// Export validation state for parent components
	const _isValid = $derived(isPasswordValid && passwordConfirmation?.trim() && passwordsMatch);

	export function isValid() {
		return _isValid;
	}
</script>

<div class="password-confirmation">
	<Textbox
		bind:value={password}
		label="Passwort"
		type="password"
		{autocomplete}
		error={passwordError}
		helperText={passwordHelperText}
		onblur={() => {
			passwordTouched = true;
		}}
	/>
	<Textbox
		bind:value={passwordConfirmation}
		label="Passwort bestätigen"
		type="password-non-visible"
		{autocomplete}
		error={passwordConfirmationError}
		oninput={() => {
			passwordConfirmationTouched = true;
		}}
		onblur={() => {
			passwordConfirmationTouched = true;
		}}
	/>
</div>

<style lang="postcss">
	.password-confirmation {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
</style>
