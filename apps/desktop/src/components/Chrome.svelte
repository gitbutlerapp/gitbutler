<script lang="ts">
	import ChromeNavigation from '$components/ChromeNavigation.svelte';
	import type { Snippet } from 'svelte';

	const { children, projectId }: { children: Snippet; projectId: string } = $props();
</script>

<ChromeNavigation {projectId} />
<div class="chrome">
	{@render children()}
</div>

<style>
	.chrome {
		display: flex;
		height: 100%;
		align-items: self-start;
		flex-shrink: 0;
		user-select: none;
		position: relative;
		background-color: var(--clr-bg-2);
	}
</style>
