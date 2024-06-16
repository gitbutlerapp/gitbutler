<script lang="ts">
	export let name = '';

	export let small = false;
	export let disabled = false;
	export let value = '';
	export let id = '';
	export let checked = false;
</script>

<input
	on:click|stopPropagation
	on:change
	on:input
	type="radio"
	class="radio"
	class:small
	{id}
	{value}
	{name}
	{disabled}
	{checked}
/>

<style lang="postcss">
	.radio {
		appearance: none;
		cursor: pointer;
		width: 16px;
		height: 16px;
		border-radius: 16px;
		background-color: var(--clr-bg-1);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* not checked */
		&:hover,
		&:focus {
			outline: none;
			box-shadow: inset 0 0 0 1px var(--clr-border-1);

			&::after {
				opacity: 0.2;
				transform: scale(0.7);
			}
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
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				opacity: 1;
				transform: scale(1);
				background-color: var(--clr-core-ntrl-100);
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			top: 4px;
			left: 4px;
			width: calc(100% - 8px);
			height: calc(100% - 8px);
			border-radius: 16px;
			background-color: var(--clr-scale-ntrl-0);
			transition: background-color var(--transition-fast);
			transform: scale(0.5);
			opacity: 0;
		}

		/* modifiers */

		&.small {
			width: 14px;
			height: 14px;
		}
	}
</style>
