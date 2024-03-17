<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	export let name = '';

	export let small = false;
	export let disabled = false;
	export let checked = false;
	export let value = '';
	export let indeterminate = false;

	let input: HTMLInputElement;
	const dispatch = createEventDispatcher<{ change: boolean }>();

	$: if (input) input.indeterminate = indeterminate;
</script>

<input
	bind:this={input}
	bind:checked
	on:click|stopPropagation
	on:change={() => {
		dispatch('change', checked);
	}}
	type="checkbox"
	class="checkbox"
	class:small
	{value}
	id={name}
	{name}
	{disabled}
/>

<style lang="postcss">
	.checkbox {
		appearance: none;
		cursor: pointer;
		width: var(--size-16);
		height: var(--size-16);
		flex-shrink: 0;
		border-radius: var(--radius-s);
		background-color: var(--clr-theme-container-light);
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
			box-shadow: inset 0 0 0 1px
				color-mix(in srgb, var(--clr-theme-container-outline-pale), var(--darken-mid));
			outline: none;

			&::after {
				opacity: 0.8;
				transform: scale(0.8);
			}
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-theme-scale-ntrl-60);
			border-color: none;
		}

		&:indeterminate {
			background-color: var(--clr-theme-container-pale);

			&::before {
				content: '';
				position: absolute;
				width: 50%;
				height: 2px;
				background-color: var(--clr-theme-scale-ntrl-30);
				top: 50%;
				left: 50%;
				transform: translate(-50%, -50%);
			}
		}

		/* checked */
		&:checked {
			background-color: var(--clr-theme-pop-element);
			box-shadow: inset 0 0 0 1px var(--clr-theme-pop-element);

			&:hover {
				background-color: color-mix(in srgb, var(--clr-theme-pop-element), var(--darken-mid));
			}

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				opacity: 1;
				filter: brightness(2);
				transform: scale(1);
			}
		}

		&::after {
			content: '';
			position: absolute;
			width: 100%;
			height: 100%;
			border-radius: var(--radius-s);
			transition:
				opacity var(--transition-fast),
				transform var(--transition-fast);
			opacity: 0;
		}

		/* tick element */
		&:not(:indeterminate)::after {
			background-image: url('data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAiIGhlaWdodD0iNyIgdmlld0JveD0iMCAwIDEwIDciIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+CjxwYXRoIGQ9Ik05IDEuNUw0LjkyMTM5IDUuNzQ4NTVDNC41Mjc4MyA2LjE1ODUxIDMuODcyMTcgNi4xNTg1MSAzLjQ3ODYxIDUuNzQ4NTZMMSAzLjE2NjY3IiBzdHJva2U9IiNBNUE1QTUiIHN0cm9rZS13aWR0aD0iMS41Ii8+Cjwvc3ZnPgo=');
			background-position: center;
			background-size: 80%;
			background-repeat: no-repeat;
		}

		/* modifiers */

		&.small {
			width: var(--size-14);
			height: var(--size-14);
		}
	}
</style>
