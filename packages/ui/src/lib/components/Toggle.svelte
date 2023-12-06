<script lang="ts">
	export let name = '';

	export let small = false;
	export let disabled = false;
	export let checked = false;
	export let value = '';
</script>

<input
	on:click|stopPropagation
	on:change
	type="checkbox"
	class="toggle"
	class:small
	{value}
	{name}
	{disabled}
	bind:checked
/>

<style lang="postcss">
	.toggle {
		appearance: none;
		width: calc(var(--space-24) + var(--space-2));
		height: var(--space-16);
		border-radius: var(--space-16);
		background-color: var(--clr-theme-container-mid);
		box-shadow: inset 0 0 0 1px var(--clr-theme-container-outline-light);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			opacity var(--transition-fast),
			transform var(--transition-fast);
		position: relative;

		/* not checked */
		&:hover,
		&:focus {
			background-color: var(--clr-theme-container-dark);
			outline: none;
		}

		&:focus {
			box-shadow: inset 0 0 0 1px var(--clr-theme-container-outline-sub);
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-theme-scale-ntrl-60);
			border-color: none;
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&:hover {
				background-color: var(--clr-theme-pop-element-dim);
				box-shadow: inset 0 0 0 1px var(--clr-theme-pop-dim);
			}

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				transform: translateX(var(--space-10));
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			top: var(--space-2);
			left: var(--space-2);
			width: var(--space-12);
			height: var(--space-12);
			border-radius: var(--space-12);
			background-color: var(--clr-core-ntrl-100);
			transition:
				background-color var(--transition-fast),
				transform var(--transition-medium);
		}

		/* modifiers */

		&.small {
			width: var(--space-24);
			height: var(--space-14);

			&:after {
				width: var(--space-10);
				height: var(--space-10);
			}
		}
	}
</style>
