<script lang="ts">
	interface Props {
		small?: boolean;
		disabled?: boolean;
		checked?: boolean;
		value?: string;
		id?: string;
		onclick?: (event: any) => void;
	}

	let {
		small = false,
		disabled = false,
		checked = $bindable(false),
		value = '',
		id = '',
		onclick
	}: Props = $props();
</script>

<input
	bind:checked
	{onclick}
	type="checkbox"
	class="toggle"
	class:small
	{value}
	{id}
	{disabled}
/>

<style lang="postcss">
	.toggle {
		appearance: none;
		cursor: pointer;
		width: 26px;
		height: 16px;
		border-radius: 16px;
		background-color: var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* not checked */
		&:hover,
		&:focus {
			background-color: var(--clr-border-1);
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-scale-ntrl-60);
			border-color: none;
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-hover);
			}

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				transform: translateX(10px);
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			top: 2px;
			left: 2px;
			width: 12px;
			height: 12px;
			border-radius: 12px;
			background-color: var(--clr-core-ntrl-100);
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
