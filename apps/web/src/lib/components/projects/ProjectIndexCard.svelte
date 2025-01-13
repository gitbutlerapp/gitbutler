<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getProjectByRepositoryId } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);

	const project = getProjectByRepositoryId(appState, projectService, projectId);
</script>

<Loading loadable={project.current}>
	{#snippet children(project)}
		<a href="/{project.owner}/{project.name}">
			<div class="card">
				<p>{project.owner}/{project.name}</p>
				<p>{project.createdAt}</p>
			</div>
		</a>
	{/snippet}
</Loading>
