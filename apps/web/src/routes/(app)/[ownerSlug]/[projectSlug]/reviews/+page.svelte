<script lang="ts">
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import { featureShowProjectPage } from '$lib/featureFlags';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReviewsForRepository } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { map } from '@gitbutler/shared/network/loadable';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	const branchService = getContext(BranchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);
	const projectService = getContext(ProjectService);
	const repositoryIdLookupService = getContext(RepositoryIdLookupService);

	const brancheses = $derived(
		getBranchReviewsForRepository(appState, branchService, data.ownerSlug, data.projectSlug)
	);

	let settingsButtonMarker = $state<HTMLElement>();

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug, {
			element: settingsButtonMarker
		})
	);

	const project = $derived(
		map(repositoryId.current, (repositoryId) =>
			getProjectByRepositoryId(appState, projectService, repositoryId, {
				element: settingsButtonMarker
			})
		)
	);
</script>

<svelte:head>
	<title>Review: {data.ownerSlug}/{data.projectSlug}</title>
</svelte:head>

<Loading loadable={brancheses?.current}>
	{#snippet children(brancheses)}
		<div class="title">
			<div class="text">Branches shared for review</div>
			<Badge>{brancheses.length || 0}</Badge>
		</div>

		<table class="commits-table">
			<thead>
				<tr>
					<th><div>Status</div></th>
					<th><div>Name</div></th>
					<th><div>UUID</div></th>
					<th><div>Branch commits</div></th>
					<th><div>Last update</div></th>
					<th><div>Authors</div></th>
					<th><div>Version</div></th>
				</tr>
			</thead>
			<tbody class="pretty">
				{#each brancheses as branches, i}
					{#each branches as branch, j}
						<BranchIndexCard
							linkParams={data}
							uuid={branch.uuid}
							roundedTop={j === 0 && i !== 0}
							roundedBottom={j === branches.length - 1}
						/>
					{/each}
				{/each}
			</tbody>
		</table>
	{/snippet}
</Loading>

{#if !$featureShowProjectPage}
	<div bind:this={settingsButtonMarker}></div>
	<Loading loadable={project?.current}>
		{#snippet children(project)}
			{#if project.permissions.canWrite}
				<div class="project-settings">
					<Button onclick={() => goto(routes.projectPath(data))}>Project settings</Button>
				</div>
			{/if}
		{/snippet}
	</Loading>
{/if}

<style>
	.title {
		display: flex;
		align-items: center;
		margin-bottom: 1.5rem;
		gap: 6px;
	}
	.title > .text {
		font-weight: bold;
	}

	.project-settings {
		margin-top: 1rem;
	}
</style>
