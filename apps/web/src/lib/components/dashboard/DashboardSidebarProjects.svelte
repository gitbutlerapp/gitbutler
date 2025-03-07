<script lang="ts">
	import DashboardSidebarProject from '$lib/components/dashboard/DashboardSidebarProject.svelte';
	import { WebState } from '$lib/redux/store.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { OrganizationService } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import { getAllUserProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';

	const webState = getContext(WebState);
	const organizationService = getContext(OrganizationService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const userProjects = $derived(username !== undefined ? getAllUserProjects(username) : undefined);

	const organizations = getOrganizations(webState, organizationService);
</script>

<div class="group">
	<p class="text-13 text-bold title">{username}</p>
	{#each userProjects?.current || [] as project}
		<DashboardSidebarProject repositoryId={project.id} />
	{/each}
</div>

{#each organizations.current as organization}
	<div class="group">
		<Loading loadable={organization}>
			{#snippet children(organization)}
				<p class="text-13 text-bold title">{organization.name}</p>
				{#each organization.projectRepositoryIds || [] as repositoryId}
					<DashboardSidebarProject {repositoryId} />
				{/each}
			{/snippet}
		</Loading>
	</div>
{/each}

<style lang="postcss">
	.title {
		padding: 14px 18px;
	}

	.group {
		width: 100%;
		border-bottom: 1px solid var(--clr-border-2);

		padding-bottom: 12px;

		&:last-child {
			border-bottom: none;
		}
	}
</style>
