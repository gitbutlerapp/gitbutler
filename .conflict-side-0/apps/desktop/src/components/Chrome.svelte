<script lang="ts">
	import ChromeHeader from '$components/ChromeHeader.svelte';
	import ChromeSidebar from '$components/ChromeSidebar.svelte';
	import type { Snippet } from 'svelte';

	const {
		projectId,
		children,
		sidebarDisabled = false
	}: { projectId: string; children: Snippet; sidebarDisabled?: boolean } = $props();
</script>

<div class="chrome">
	<ChromeHeader {projectId} actionsDisabled={sidebarDisabled} />
	<div class="chrome-body">
		<ChromeSidebar disabled={sidebarDisabled} />
		<div class="chrome-content">
			{@render children()}
		</div>
	</div>
</div>

<style>
	.chrome {
		display: flex;
		flex: 1;
		background-color: var(--clr-bg-2);
		flex-direction: column;
		max-width: 100%;
	}

	.chrome-body {
		display: flex;
		overflow: hidden;
		flex-grow: 1;
		height: 100%;
	}

	.chrome-content {
		display: flex;
		flex-grow: 1;
		padding: 0 14px 14px 0;
		align-items: self-start;
		user-select: none;
		overflow: auto;
	}
</style>
