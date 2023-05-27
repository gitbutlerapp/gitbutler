<script lang="ts">
	import { stores, log, toasts } from '$lib';
	import { CloudApi, type Project } from '$lib/api';
	import { Login, Checkbox } from '$lib/components';
	import { createEventDispatcher, onMount } from 'svelte';

	export let project: Project;
	const user = stores.user;
	const cloud = CloudApi();

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
			log.error(`Failed to update project sync status: ${error}`);
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
		<div class="flex flex-row items-center justify-between rounded-lg border border-zinc-600 p-2">
			<div class="flex flex-row space-x-3">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
					stroke-width="1.5"
					stroke="white"
					class="h-6 w-6"
				>
					<path
						stroke-linecap="round"
						stroke-linejoin="round"
						d="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5a4.5 4.5 0 01-1.41-8.775 5.25 5.25 0 0110.233-2.33 3 3 0 013.758 3.848A3.752 3.752 0 0118 19.5H6.75z"
					/>
				</svg>
				<div class="flex flex-row">
					<form class="flex items-center gap-1">
						<Checkbox
							name="sync"
							disabled={$user === undefined}
							checked={project.api?.sync || false}
							on:change={onSyncChange}
						/>
						<label for="sync">Enable GitButler Cloud</label>
					</form>
				</div>
			</div>
		</div>
	{:else}
		<Login />
	{/if}
</section>
