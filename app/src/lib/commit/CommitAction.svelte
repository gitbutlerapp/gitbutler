<script lang="ts">
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { type Snippet } from 'svelte';

	interface Props {
		lines: Snippet;
		action: Snippet;
	}

	const { lines, action }: Props = $props();

	let isNotInViewport = $state(false);
</script>

<div
	class="action-row sticky"
	class:not-in-viewport={isNotInViewport}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry.isIntersecting) {
				console.log('entry.isIntersecting', entry.isIntersecting);
				isNotInViewport = false;
			} else {
				isNotInViewport = true;
				console.log('entry.isIntersecting', entry.isIntersecting);
			}
		},
		options: {
			root: null,
			rootMargin: '-1px',
			threshold: 1
		}
	}}
>
	<div>
		{@render lines()}
	</div>
	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.action-row {
		position: relative;
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		overflow: hidden;

		transition: border-top var(--transition-fast);
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 10px;
		padding-right: 14px;
		padding-bottom: 10px;
	}

	/* MODIFIERS */
	.sticky {
		z-index: var(--z-ground);
		position: sticky;
		bottom: 0;
	}

	.not-in-viewport {
		box-shadow: 0 0 0 1px var(--clr-border-2);
	}
</style>
