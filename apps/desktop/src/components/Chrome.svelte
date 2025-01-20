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
		flex-grow: 1;
		flex-direction: column;
		max-height: 100vh;
	}

	.wrapper {
		display: flex;
		flex-grow: 1;
		height: 100%;
		overflow-y: scroll;
	}

	.content {
		display: flex;
		flex-grow: 1;
		padding: 16px 0 0 16px;
		align-items: self-start;
		user-select: none;
		background-color: var(--clr-bg-2);
	}
</style>
