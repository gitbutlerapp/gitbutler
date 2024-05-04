<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	import { createEventDispatcher } from 'svelte';

	export let small = false;
	export let disabled = false;
	export let checked = false;
	export let value = '';
	export let help = '';
	export let id = '';

	let input: HTMLInputElement;
	const dispatch = createEventDispatcher<{ change: boolean }>();
</script>

<input
	bind:this={input}
	bind:checked
	on:click|stopPropagation
	on:change={() => {
		dispatch('change', checked);
	}}
	type="checkbox"
	class="toggle"
	class:small
	{value}
	{id}
	{disabled}
	use:tooltip={help}
/>

<style lang="postcss">
	.toggle {
		appearance: none;
		cursor: pointer;
		width: calc(var(--size-24) + var(--size-2));
		height: var(--size-16);
		border-radius: var(--size-16);
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
			background-color: oklch(from var(--clr-border-2) var(--hover-state-ratio) c h);
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
				background-color: oklch(from var(--clr-theme-pop-element) var(--hover-state-ratio) c h);
			}

			&:disabled {
				pointer-events: none;
				opacity: 0.4;
				cursor: not-allowed;
			}

			&::after {
				transform: translateX(var(--size-10));
			}
		}

		/* tick element */
		&::after {
			content: '';
			position: absolute;
			top: var(--size-2);
			left: var(--size-2);
			width: var(--size-12);
			height: var(--size-12);
			border-radius: var(--size-12);
			background-color: var(--clr-core-ntrl-100);
			transition:
				background-color var(--transition-fast),
				transform var(--transition-medium);
		}

		/* modifiers */

		&.small {
			width: var(--size-24);
			height: var(--size-14);

			&:after {
				width: var(--size-10);
				height: var(--size-10);
			}
		}
	}
</style>
