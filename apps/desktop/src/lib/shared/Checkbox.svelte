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
		width: 16px;
		height: 16px;
		flex-shrink: 0;
		border-radius: var(--radius-s);
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

			&::after {
				opacity: 0.8;
				transform: scale(0.8);
			}
		}

		&:disabled {
			pointer-events: none;
			opacity: 0.3;
			cursor: not-allowed;
			background-color: var(--clr-scale-ntrl-60);
			border-color: none;
		}

		&:indeterminate {
			background-color: var(--clr-bg-2);

			&::before {
				content: '';
				position: absolute;
				width: 50%;
				height: 2px;
				background-color: var(--clr-scale-ntrl-30);
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
				background-color: var(--clr-theme-pop-element-hover);
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
			width: 14px;
			height: 14px;
		}
	}
</style>
