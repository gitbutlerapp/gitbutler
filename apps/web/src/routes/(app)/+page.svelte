<script lang="ts">
	import DashboardLayout from '$lib/components/dashboard/DashboardLayout.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { getRecentlyPushedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { goto } from '$app/navigation';

	const routes = getContext(WebRoutesService);
	const recentProjects = getRecentlyPushedProjects();

	$effect(() => {
		if (recentProjects.current.length >= 1) {
			const project = recentProjects.current[0];
			if (isFound(project)) {
				goto(
					routes.projectReviewUrl({
						ownerSlug: project.value.owner,
						projectSlug: project.value.slug
					})
				);
			}
		}
	});
</script>

<DashboardLayout>
	<p>You have no recent projects!</p>
</DashboardLayout>
