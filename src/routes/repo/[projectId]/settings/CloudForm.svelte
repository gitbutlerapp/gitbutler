<script lang="ts">
	import { toasts } from '$lib';
	import { getCloudApiClient } from '$lib/api/cloud/api';
	import type { Project } from '$lib/api/ipc/projects';
	import { userStore } from '$lib/stores/user';
	import { Login, Checkbox } from '$lib/components';
	import { createEventDispatcher, onMount } from 'svelte';

	export let project: Project;
	const user = userStore;
	const cloud = getCloudApiClient();

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();

	onMount(async () => {
		if (!project.api) return;
		const u = await user.load();
		if (!u) return;
		const cloudProject = await cloud.projects.get(u.access_token, project.api.repository_id);
		if (cloudProject === project.api) return;
		dispatch('updated', { ...project, api: { ...cloudProject, sync: project.api.sync } });
	});

	const onSyncChange = async (event: Event) => {
		if ($user === null) return;

		const target = event.target as HTMLInputElement;
		const sync = target.checked;

		try {
			const cloudProject =
				project.api ??
				(await cloud.projects.create($user.access_token, {
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			dispatch('updated', { ...project, api: { ...cloudProject, sync } });
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	};
</script>

<section class="space-y-2">
	<header>
		<h2 class="text-xl">GitButler Cloud</h2>
		<span class="text-text-subdued">
			Sync with GitButler secure cloud for AI features, team features, and more.
		</span>
	</header>

	{#if $user}
		<div
			class="flex flex-row items-center justify-between rounded-lg border border-light-400 p-2 dark:border-dark-500"
		>
			<div class="flex flex-row space-x-3">
				<div class="flex flex-row">
					<form class="flex items-center gap-1">
						<Checkbox
							name="sync"
							disabled={$user === undefined}
							checked={project.api?.sync || false}
							on:change={onSyncChange}
						/>
						<label class="ml-2" for="sync">Enable GitButler Cloud</label>
					</form>
				</div>
			</div>
		</div>
	{:else}
		<Login />
	{/if}
</section>
