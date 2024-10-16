<script lang="ts">
	import { intersectionObserver } from '$lib/utils/intersectionObserver';

	interface Props {
		title: string;
	}

	const { title }: Props = $props();

	let isIntersected = $state(false);
</script>

<div
	class="group-header"
	class:intersected={isIntersected}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry?.isIntersecting) {
				isIntersected = false;
			} else {
				isIntersected = true;
			}
		},
		options: {
			root: null,
			rootMargin: '-100% 0px 0px 0px',
			threshold: 0
		}
	}}
>
	<h3 class="text-12 text-semibold">{title}</h3>
</div>

<style>
	.group-header {
		z-index: var(--z-lifted);
		position: sticky;
		top: -10px;
		padding: 10px 14px;
		color: var(--clr-text-2);
		background-color: var(--clr-bg-1);
	}

	.group-header.intersected {
		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
