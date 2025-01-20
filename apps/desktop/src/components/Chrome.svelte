<script lang="ts">
	import Header from '$components/ChromeHeader.svelte';
	import Sidebar from '$components/ChromeSidebar.svelte';
	import type { Snippet } from 'svelte';

	const { children, projectId }: { children: Snippet; projectId: string } = $props();
</script>

<div class="container">
	<Header />
	<div class="content">
		<Sidebar {projectId} />
		<div class="chrome">
			{@render children()}
		</div>
	</div>
</div>

<style>
	.container {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		height: 100%;

		.content {
			display: flex;
			flex-grow: 1;
			height: 100%;

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
		}
	}
</style>
