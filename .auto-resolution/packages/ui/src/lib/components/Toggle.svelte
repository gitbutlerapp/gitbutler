<script lang="ts">
	import { focusable } from '$lib/focus/focusable';

	interface Props {
		small?: boolean;
		disabled?: boolean;
		checked?: boolean;
		value?: string;
		id?: string;
		testId?: string;
		onclick?: (e: MouseEvent) => void;
		onchange?: (checked: boolean) => void;
	}

	let {
		testId,
		small,
		disabled,
		checked = $bindable(),
		value,
		id,
		onclick,
		onchange
	}: Props = $props();
</script>

<input
	data-testid={testId}
	bind:checked
	onclick={(e) => {
		e.stopPropagation();
		onclick?.(e);
	}}
	onchange={(e) => onchange?.(e.currentTarget.checked)}
	type="checkbox"
	class="toggle"
	class:small
	{value}
	{id}
	{disabled}
	use:focusable={{ button: true }}
/>

<style lang="postcss">
	.toggle {
		appearance: none;
		position: relative;
		flex-shrink: 0;
		width: 26px;
		height: 16px;
		border-radius: 16px;
		background-color: var(--clr-border-2);
		cursor: pointer;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);

		/* not checked */
		&:hover,
		&:focus {
			background-color: var(--clr-border-1);
		}

		&:disabled {
			border-color: none;
			background-color: var(--clr-scale-ntrl-60);
			cursor: not-allowed;
			opacity: 0.3;
			pointer-events: none;
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-hover);
			}

			&:disabled {
				cursor: not-allowed;
				opacity: 0.4;
				pointer-events: none;
			}

			&::after {
				transform: translateX(10px);
			}
		}

		/* tick element */
		&::after {
			position: absolute;
			top: 2px;
			left: 2px;
			width: 12px;
			height: 12px;
			border-radius: 12px;
			background-color: var(--clr-core-ntrl-100);
			content: '';
			transition:
				background-color var(--transition-fast),
				transform var(--transition-medium);
		}

		/* modifiers */

		&.small {
			width: 24px;
			height: 14px;

			&:after {
				width: 10px;
				height: 10px;
			}
		}
	}
</style>
