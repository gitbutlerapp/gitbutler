<script lang="ts">
	import DashboardSidebar from '$lib/components/dashboard/DashboardSidebar.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet;
	};

	const { children }: Props = $props();
</script>

<div class="dashboard-layout">
	<div>
		<DashboardSidebar />
	</div>
	<div>
		{@render children()}
	</div>
</div>

<style lang="postcss">
	.dashboard-layout {
		display: grid;
		grid-template-columns: 4fr 12fr;
		gap: 20px;
	}
</style>
