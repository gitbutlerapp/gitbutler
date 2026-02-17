<script lang="ts" module>
	export interface Props {
		id?: string;
		name?: string;
		small?: boolean;
		disabled?: boolean;
		checked?: boolean;
		value?: string;
		indeterminate?: boolean;
		onclick?: (e: MouseEvent) => void;
		onchange?: (
			e: (Event | KeyboardEvent) & {
				currentTarget: EventTarget & HTMLInputElement;
			},
		) => void;
	}
</script>

<script lang="ts">
	let input = $state<HTMLInputElement>();

	let {
		id,
		name,
		small = false,
		disabled = false,
		checked = $bindable(),
		value = "",
		indeterminate = false,
		onclick,
		onchange,
	}: Props = $props();

	function getCheckmarkColor(): string {
		if (disabled) return "var(--clr-text-2)";
		if (!checked) return "var(--clr-text-2)";
		return "var(--clr-theme-pop-on-element)";
	}

	const checkmarkColor = $derived(getCheckmarkColor());

	function handleClick(e: MouseEvent) {
		e.stopPropagation();
		onclick?.(e);
	}

	function handleChange(e: Event & { currentTarget: EventTarget & HTMLInputElement }) {
		e.stopPropagation();
		onchange?.(e);
	}

	function handleKeydown(e: KeyboardEvent & { currentTarget: EventTarget & HTMLInputElement }) {
		if (e.key === "Enter") {
			e.preventDefault();
			e.stopPropagation();
			checked = !checked;
			onchange?.(e);
		}
	}

	$effect(() => {
		if (input) input.indeterminate = indeterminate;
	});
</script>

<div
	class="checkbox-wrapper"
	class:checked
	class:small
	class:disabled
	class:indeterminate
	style:--checkmark-color={checkmarkColor}
>
	<div class="checkbox-checkmark">
		{#if !indeterminate}
			<!-- This is a tick icon, it will be shown when the checkbox is checked -->
			<svg
				width="10"
				height="10"
				viewBox="0 0 10 10"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					d="M9 2.5L4.92139 6.74855C4.52783 7.15851 3.87217 7.15851 3.47861 6.74856L1 4.16667"
					stroke="var(--checkmark-color)"
					stroke-width="1.5"
				/>
			</svg>
		{:else}
			<svg width="8" height="2" viewBox="0 0 8 2" fill="none" xmlns="http://www.w3.org/2000/svg">
				<path d="M8 1L0 1" stroke="var(--checkmark-color)" stroke-width="1.5" />
			</svg>
		{/if}
	</div>

	<input
		bind:this={input}
		bind:checked
		tabindex="0"
		onclick={handleClick}
		onchange={handleChange}
		onkeydown={handleKeydown}
		type="checkbox"
		class="checkbox-input"
		{value}
		id={id ?? name}
		{name}
		{disabled}
	/>
</div>

<style lang="postcss">
	.checkbox-wrapper {
		--border-width: 1px;
		--disabled-opacity: 50%;

		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		border-radius: var(--radius-s);
		background-color: var(--clr-bg-1);
		box-shadow: inset 0 0 0 var(--border-width) var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			box-shadow var(--transition-fast);

		&.small {
			width: 14px;
			height: 14px;
		}

		/* Unchecked states */
		&:not(.checked):not(.disabled):hover,
		:global(label:hover) &:not(.checked):not(.disabled) {
			box-shadow: inset 0 0 0 var(--border-width) var(--clr-border-1);

			& .checkbox-checkmark {
				opacity: 1;
			}
		}

		&:not(.checked):not(.disabled):has(.checkbox-input:focus-visible) {
			outline: 2px solid var(--clr-theme-pop-element);
			outline-offset: -2px;
		}

		/* Checked states */
		&.checked:not(.disabled) {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 var(--border-width) var(--clr-theme-pop-element);

			& .checkbox-checkmark {
				transform: scale(1);
				opacity: 1;
			}

			&:hover,
			:global(label:hover) & {
				background-color: var(--hover-pop);
				box-shadow: inset 0 0 0 var(--border-width) var(--hover-pop);
			}

			&:has(.checkbox-input:focus-visible) {
				outline: 2px solid color-mix(in srgb, var(--clr-theme-pop-element) 80%, var(--clr-text-1));
				outline-offset: -2px;
			}
		}

		/* Disabled states */
		&.disabled .checkbox-input {
			cursor: not-allowed;
		}

		&.disabled:not(.checked) {
			background-color: color-mix(
				in srgb,
				var(--clr-border-2) var(--disabled-opacity),
				var(--clr-bg-1)
			);
			box-shadow: inset 0 0 0 var(--border-width)
				color-mix(in srgb, var(--clr-border-2) var(--disabled-opacity), var(--clr-bg-1));
		}

		&.disabled.checked .checkbox-checkmark {
			transform: scale(1);
			opacity: 1;
		}

		&.disabled.checked {
			background-color: color-mix(
				in srgb,
				var(--clr-theme-pop-element) var(--disabled-opacity),
				var(--clr-bg-1)
			);
			box-shadow: inset 0 0 0 var(--border-width)
				color-mix(in srgb, var(--clr-theme-pop-element) var(--disabled-opacity), var(--clr-bg-1));
		}
	}

	.checkbox-checkmark {
		display: flex;
		transform: scale(0.8);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}

	.checkbox-input {
		appearance: none;
		z-index: 1;
		position: absolute;
		inset: 0;
		border-radius: var(--radius-s);
		cursor: pointer;
	}
</style>
