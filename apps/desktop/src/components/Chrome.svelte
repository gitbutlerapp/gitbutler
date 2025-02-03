<script lang="ts">
	import Header from '$components/ChromeHeader.svelte';
	import Sidebar from '$components/ChromeSidebar.svelte';
	import type { Snippet } from 'svelte';

	const { children }: { children: Snippet } = $props();
</script>

<div class="chrome">
	<Header />
	<div class="wrapper">
		<Sidebar />
		<div class="content">
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

	.wrapper {
		display: flex;
		overflow: hidden;
		flex-grow: 1;
		height: 100%;
		max-width: 100vw;
	}

	.content {
		display: flex;
		flex-grow: 1;
		padding: 0 16px 16px 0;
		align-items: self-start;
		user-select: none;
		overflow: hidden;
	}
</style>
