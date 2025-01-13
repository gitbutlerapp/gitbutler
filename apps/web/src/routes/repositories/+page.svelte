<script lang="ts">
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { ProjectService } from '@gitbutler/shared/organizations/projectService';
	import { getAllUserProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const username = $derived($user?.login);

	const userProjects = $derived(getAllUserProjects(username ?? 'bla', appState, projectService));
</script>

<h2>Your projects:</h2>

{#each userProjects.current as project (project.id)}
	<Loading loadable={project}>
		{#snippet children(project)}
			<div>
				<p>{project.name}</p>
				<p>{project.createdAt}</p>
			</div>
		{/snippet}
	</Loading>
{/each}

<style>
</style>
