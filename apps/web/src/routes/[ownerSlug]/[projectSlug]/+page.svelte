<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
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

	let deletingState = $state<'inert' | 'loading' | 'complete'>('inert');

	async function deleteProject(repositoryId: string) {
		if (!confirm('Are you sure you want to delete this project?')) {
			return;
		}

		deletingState = 'loading';

		try {
			await projectService.deleteProject(repositoryId);
		} finally {
			goto(routes.projectsPath());
			deletingState = 'complete';
		}
	}
</script>

<h2>Project page: {data.ownerSlug}/{data.projectSlug}</h2>

<div class="flow">
	<Button style="pop" onclick={() => goto(routes.projectReviewPath(data))}>Project Reviews</Button>
	<hr />
	<p data-info="https://youtu.be/siwpn14IE7E">The danger zone</p>
	<Loading loadable={repositoryId.current}>
		{#snippet children(repositoryId)}
			<Button
				style="error"
				onclick={() => deleteProject(repositoryId)}
				loading={deletingState === 'loading'}>Delete</Button
			>
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
