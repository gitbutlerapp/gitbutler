<script lang="ts" module>
	export interface Props {
		name?: string;
		small?: boolean;
		disabled?: boolean;
		checked?: boolean;
		value?: string;
		indeterminate?: boolean;
		onclick?: (e: MouseEvent) => void;
		onchange?: (
			e: Event & {
				currentTarget: EventTarget & HTMLInputElement;
			}
		) => void;
	}
</script>

<script lang="ts">
	let input = $state<HTMLInputElement>();

	let {
		name,
		small = false,
		disabled = false,
		checked = $bindable(),
		value = '',
		indeterminate = false,
		onclick,
		onchange
	}: Props = $props();

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
	style:--checkmark-color="var(--clr-theme-pop-on-element)"
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
		onclick={(e) => {
			e.stopPropagation();
			onclick?.(e);
		}}
		onchange={(e) => {
			e.stopPropagation();
			onchange?.(e);
		}}
		onkeydown={(e) => {
			if (e.key === 'Enter') {
				e.preventDefault();
				e.stopPropagation();

				checked = !checked;
				onchange?.(e);
			}
		}}
		type="checkbox"
		class="checkbox-input"
		{value}
		id={name}
		{name}
		{disabled}
	/>
</div>

<style lang="postcss">
	.checkbox-wrapper,
	.checkbox-input {
		border-radius: var(--radius-s);
	}

	.checkbox-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		background-color: var(--clr-bg-1);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		/* NOT CHECKED */
		&:not(.checked):not(.disabled) {
			& .checkbox-checkmark {
				--checkmark-color: var(--clr-text-2);
			}
		}
		/* NOT CHECKED. HOVER */
		&:not(.checked):not(.disabled):hover {
			box-shadow: inset 0 0 0 1px var(--clr-border-1);
			& .checkbox-checkmark {
				opacity: 1;
			}
		}

		/* CHECKED */
		&:not(.disabled).checked {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);
			& .checkbox-checkmark {
				transform: scale(1);
				opacity: 1;
			}
		}
		/* CHECKED. HOVER */
		&:not(.disabled).checked:hover {
			background-color: var(--clr-theme-pop-element-hover);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element-hover);
			& .checkbox-checkmark {
				opacity: 1;
			}
		}

		/* CURSOR */
		&:not(.disabled) {
			& .checkbox-input {
				cursor: pointer;
			}
		}
		&.disabled {
			& .checkbox-input {
				cursor: not-allowed;
			}
		}

		/* DISABLED */
		&:not(.checked).disabled {
			background-color: color-mix(in srgb, var(--clr-scale-ntrl-70) 50%, var(--clr-bg-1));
			box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--clr-scale-ntrl-70) 50%, var(--clr-bg-1));
		}
		/* DISABLED. CHECKED */
		&.disabled.checked {
			--checkmark-color: var(--clr-text-2);
			background-color: color-mix(in srgb, var(--clr-theme-pop-element) 50%, var(--clr-bg-1));
			box-shadow: inset 0 0 0 1px
				color-mix(in srgb, var(--clr-theme-pop-element) 50%, var(--clr-bg-1));

			& .checkbox-checkmark {
				transform: scale(1);
				opacity: 1;
			}
		}

		/* MODIFIERS */
		&.small {
			width: 14px;
			height: 14px;
		}
	}

	.checkbox-checkmark {
		display: flex;
		transform: scale(0.8);
		opacity: 0;
		pointer-events: none; /* Prevents the checkmark from blocking clicks */
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}

	.checkbox-input {
		appearance: none;
		z-index: 1;
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
	}
</style>
