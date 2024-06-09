<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import { createEventDispatcher } from 'svelte';

	export let isLast = false;
	export let isFirst = false;
	export let isMiddle = false;

	const dispatch = createEventDispatcher<{ click: void }>();
</script>

<div class="container" class:is-last={isLast} class:is-first={isFirst} class:is-middle={isMiddle}>
	<div class="hover-target">
		<Button
			style="ghost"
			outline
			solidBackground
			icon="plus-small"
			size="tag"
			width={26}
			help="Insert empty commit"
			helpShowDelay={500}
			on:click={() => dispatch('click')}
		/>
	</div>
</div>

<style lang="postcss">
	.container {
		--height: 14px;
		--container-margin: calc(var(--height) / 2 * -1);

		position: relative;
		width: 100%;
		height: var(--height);
		z-index: var(--z-lifted);
		margin-top: var(--container-margin);
		margin-bottom: var(--container-margin);

		/* background-color: rgba(235, 167, 78, 0.159); */

		&:hover {
			& .hover-target {
				transition-delay: 0.08s;
				pointer-events: all;
				transform: translateY(-50%);
				opacity: 1;
			}
		}

		&:not(.is-last, .is-first) {
			&:before {
				pointer-events: none;
				content: '';
				position: absolute;
				top: 50%;
				left: 0;
				width: 100%;
				height: 1px;
				background-color: var(--clr-border-2);
				transform: translateY(-50%);
				opacity: 0;
				transition: opacity var(--transition-fast);
			}

			&:hover {
				&:before {
					opacity: 1;
				}
			}
		}
	}

	.hover-target {
		position: absolute;
		top: 50%;
		right: 24px;
		transform: translateY(-50%);
		width: fit-content;
		align-items: center;
		transform: translateY(calc(-50% + 4px));
		opacity: 0;
		pointer-events: none;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
		transition-delay: 0s;
	}

	/* MODIFIERS */

	.container.is-last {
		transform: translateY(-4px);
	}

	.container.is-first {
		transform: translateY(16px);
	}

	.container.is-middle {
		transform: translateY(6px);
	}
</style>
