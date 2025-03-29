<script lang="ts">
	interface Props {
		title: string;
	}

	const { title }: Props = $props();

	let isIntersected = $state(false);
</script>

<div class="group-header" class:intersected={isIntersected}>
	<h3 class="text-12 text-semibold">{title}</h3>
</div>

<style>
	.group-header {
		z-index: var(--z-ground);
		position: sticky;
		top: 0;
		padding: 12px 12px 8px;
		color: var(--clr-text-2);
		background-color: var(--clr-bg-2);
		border-top: 1px solid var(--clr-border-3);
		border-bottom: 1px solid var(--clr-border-3);
	}
</style>
