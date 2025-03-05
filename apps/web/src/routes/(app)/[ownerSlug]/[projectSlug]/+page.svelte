<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, map } from '@gitbutler/shared/network/loadable';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { ShareLevel } from '@gitbutler/shared/permissions';
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

	async function updatePermission(
		repositoryId: string,
		shareLevel: ShareLevel.Public | ShareLevel.Private | ShareLevel.Unlisted
	) {
		await projectService.updateProject(repositoryId, { shareLevel });
	}
</script>

<h2>Project page: {data.ownerSlug}/{data.projectSlug}</h2>

<div class="flow">
	<Button style="pop" onclick={() => goto(routes.projectReviewPath(data))}>Project Reviews</Button>
	<Loading loadable={combine([repositoryId.current, project?.current])}>
		{#snippet children([repositoryId, project])}
			{#if project.permissions.canWrite}
				<hr />
				<p data-info="https://youtu.be/siwpn14IE7E">The danger zone</p>

				<div>
					<p>This project is <b>{project.permissions.shareLevel}</b></p>

					{#if project.permissions.shareLevel !== ShareLevel.Private}
						<AsyncButton
							action={async () => await updatePermission(repositoryId, ShareLevel.Private)}
							>Make Private</AsyncButton
						>
					{/if}

					{#if project.permissions.shareLevel !== ShareLevel.Unlisted}
						<AsyncButton
							action={async () => await updatePermission(repositoryId, ShareLevel.Unlisted)}
							>Make Unlisted</AsyncButton
						>
					{/if}

					{#if project.permissions.shareLevel !== ShareLevel.Public}
						<AsyncButton
							action={async () => await updatePermission(repositoryId, ShareLevel.Public)}
							>Make Public</AsyncButton
						>
					{/if}
				</div>

				<AsyncButton style="error" action={async () => await deleteProject(repositoryId)}
					>Delete</AsyncButton
				>
			{/if}
		{/snippet}
	</Loading>
</div>

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
