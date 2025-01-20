<script lang="ts">
	import Sidebar from '$components/ChromeSidebar.svelte';
	import type { Snippet } from 'svelte';

	const { children, projectId }: { children: Snippet; projectId: string } = $props();
</script>

<Sidebar {projectId} />
<div class="chrome">
	{@render children()}
</div>

<style>
	.chrome {
		display: flex;
		flex-grow: 1;
		height: calc(100% - 32px);
		margin: 16px;
		align-items: self-start;
		user-select: none;
		position: relative;
		background-color: var(--clr-bg-2);
	}
</style>
