<script lang="ts">
	import { cleanBreadcrumbs } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';
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

	$effect(cleanBreadcrumbs);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const userProjects = $derived(
		username !== undefined ? getAllUserProjects(username, appState, projectService) : undefined
	);

	const organizations = getOrganizations(appState, organizationService);
</script>

<h2>My Projects:</h2>

<table class="projects-table">
	<thead>
		<tr>
			<th><div>Active Reviews</div></th>
			<th><div>Project</div></th>
			<th><div>Created</div></th>
			<th><div>Updated</div></th>
		</tr>
	</thead>
	<tbody>
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
	</tbody>
</table>

<style lang="postcss">
	.projects-table {
		th {
			padding: 0;
			> div {
				text-align: left;
				padding: 16px;

				border-top: 1px solid var(--clr-border-2);
				border-bottom: 1px solid var(--clr-border-2);
				overflow: hidden;
			}

			&:first-child {
				> div {
					border-left: 1px solid var(--clr-border-2);
					border-top-left-radius: var(--radius-m);
				}
			}

			&:last-child {
				> div {
					border-right: 1px solid var(--clr-border-2);
					border-top-right-radius: var(--radius-m);
				}
			}
		}
	}
</style>
