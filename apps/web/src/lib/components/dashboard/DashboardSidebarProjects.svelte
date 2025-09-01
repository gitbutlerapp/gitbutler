<script lang="ts">
	import DashboardSidebarProject from '$lib/components/dashboard/DashboardSidebarProject.svelte';
	import { WEB_STATE } from '$lib/redux/store.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ORGANIZATION_SERVICE } from '@gitbutler/shared/organizations/organizationService';
	import { getOrganizations } from '@gitbutler/shared/organizations/organizationsPreview.svelte';
	import {
		getAllUserProjects,
		getRecentlyPushedProjects
	} from '@gitbutler/shared/organizations/projectsPreview.svelte';

	const webState = inject(WEB_STATE);
	const organizationService = inject(ORGANIZATION_SERVICE);
	const userService = inject(USER_SERVICE);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const organizations = getOrganizations(webState, organizationService);

	const recentProjects = getRecentlyPushedProjects();
	const latestRecentProjects = $derived(recentProjects.current.slice(0, 3));
	const userProjects = $derived(username !== undefined ? getAllUserProjects(username) : undefined);
	const filtedUserProjects = $derived(
		(userProjects?.current || []).filter((project) =>
			latestRecentProjects.every((recentProject) => recentProject.id !== project.id)
		)
	);
</script>

{#if recentProjects.current.length > 0}
	<div class="group">
		<p class="text-13 text-bold title">Recent projects</p>
		{#each latestRecentProjects as project}
			<DashboardSidebarProject repositoryId={project.id} showOwner />
		{/each}
	</div>
{/if}

<div class="group">
	<p class="text-13 text-bold title">{username}</p>
	{#each filtedUserProjects as project}
		<DashboardSidebarProject repositoryId={project.id} inRecentSection={false} />
	{/each}
</div>

{#each organizations.current as organization}
	<div class="group">
		<Loading loadable={organization}>
			{#snippet children(organization)}
				<p class="text-13 text-bold title">{organization.name}</p>
				{#each organization.projectRepositoryIds || [] as repositoryId}
					<DashboardSidebarProject {repositoryId} inRecentSection={false} />
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

		padding-bottom: 12px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}
</style>
