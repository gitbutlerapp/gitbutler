<script lang="ts">
	import DashboardSidebarProject from '$lib/components/dashboard/DashboardSidebarProject.svelte';
	import { getRecentlyInteractedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';

	const recentProjects = getRecentlyInteractedProjects();
</script>

<div class="container">
	{#if recentProjects.current.length > 0}
		{#each recentProjects?.current || [] as project}
			<DashboardSidebarProject repositoryId={project.id} showOwner />
		{/each}
	{/if}
</div>

<style lang="postcss">
	.container {
		width: 100%;
		padding: 12px 0;
	}
</style>
