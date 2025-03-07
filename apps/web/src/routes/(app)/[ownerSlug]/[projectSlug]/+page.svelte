<script lang="ts">
	import DashboardLayout from '$lib/components/dashboard/DashboardLayout.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, map } from '@gitbutler/shared/network/loadable';
	import PermissionsSelector from '@gitbutler/shared/organizations/PermissionsSelector.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	const projectService = getContext(ProjectService);
	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);

	const project = $derived(
		map(repositoryId.current, (repositoryId) =>
			getProjectByRepositoryId(appState, projectService, repositoryId)
		)
	);

	async function deleteProject(repositoryId: string) {
		if (!confirm('Are you sure you want to delete this project?')) {
			return;
		}

		await projectService.deleteProject(repositoryId);
		goto(routes.projectsPath());
	}
</script>

<DashboardLayout>
	<h2>Project page: {data.ownerSlug}/{data.projectSlug}</h2>

	<div class="flow">
		<Button style="pop" onclick={() => goto(routes.projectReviewPath(data))}>Project Reviews</Button
		>
		<Loading loadable={combine([repositoryId.current, project?.current])}>
			{#snippet children([repositoryId, project])}
				{#if project.permissions.canWrite}
					<hr />
					<p data-info="https://youtu.be/siwpn14IE7E">The danger zone</p>

					<div>
						<p>This project is <b>{project.permissions.shareLevel}</b></p>

						<PermissionsSelector repositoryId={project.repositoryId} />
					</div>

					<AsyncButton style="error" action={async () => await deleteProject(repositoryId)}
						>Delete</AsyncButton
					>
				{/if}
			{/snippet}
		</Loading>
	</div>
</DashboardLayout>

<style lang="postcss">
	.flow {
		> :global(*) {
			margin-bottom: 16px;
		}

		> :global(*:last-child) {
			margin-bottom: 0px;
		}
	}
</style>
