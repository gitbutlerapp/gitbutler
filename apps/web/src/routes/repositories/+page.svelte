<script lang="ts">
	import OrganizationProjects from '$lib/components/projects/OrganizationProjects.svelte';
	import ProjectIndexCard from '$lib/components/projects/ProjectIndexCard.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import LoadingState from '@gitbutler/shared/network/LoadingState.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getAllUserProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const organizationService = getContext(OrganizationService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const userProjects = $derived(
		username !== undefined ? getAllUserProjects(username, appState, projectService) : undefined
	);

	const organizations = getOrganizations(appState, organizationService);
</script>

<h2>My Projects:</h2>

{#if userProjects === undefined || userProjects.current.length === 0}
	<LoadingState />
{:else}
	{#each userProjects.current as project (project.id)}
		<ProjectIndexCard projectId={project.id} />
	{/each}
{/if}

{#each organizations.current as organization (organization.id)}
	<Loading loadable={organization}>
		{#snippet children(organization)}
			<h2>{organization.slug}:</h2>
			<OrganizationProjects slug={organization.slug} />
		{/snippet}
	</Loading>
{/each}

<style>
</style>
