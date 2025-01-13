<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import LoadingState from '@gitbutler/shared/network/LoadingState.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getAllUserProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const userProjects = $derived(
		username !== undefined ? getAllUserProjects(username, appState, projectService) : undefined
	);
</script>

<h2>Your projects:</h2>
{#if userProjects === undefined || userProjects.current.length === 0}
	<LoadingState />
{:else}
	{#each userProjects.current as project (project.id)}
		<Loading loadable={project}>
			{#snippet children(project)}
				<a href="/{project.owner}/{project.name}">
					<div>
						<p>{project.name}</p>
						<p>{project.createdAt}</p>
					</div>
				</a>
			{/snippet}
		</Loading>
	{/each}
{/if}

<style>
</style>
