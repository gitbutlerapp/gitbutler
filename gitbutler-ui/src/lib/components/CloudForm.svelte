<script lang="ts">
	import { getCloudApiClient, type User } from '$lib/backend/cloud';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Link from '$lib/components/Link.svelte';
	import Login from '$lib/components/Login.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher, onMount } from 'svelte';
	import type { Project } from '$lib/backend/projects';
	import type { UserService } from '$lib/stores/user';
	import { PUBLIC_API_BASE_URL } from '$env/static/public';

	export let project: Project;
	export let user: User | undefined;
	export let userService: UserService;

	const cloud = getCloudApiClient();
	const aiGenEnabled = projectAiGenEnabled(project.id);

	const dispatch = createEventDispatcher<{
		updated: Project;
	}>();

	onMount(async () => {
		if (!project?.api) return;
		if (!user) return;
		const cloudProject = await cloud.projects.get(user.access_token, project.api.repository_id);
		if (cloudProject === project.api) return;
		dispatch('updated', { ...project, api: { ...cloudProject, sync: project.api.sync } });
	});

	const onSyncChange = async (event: CustomEvent<boolean>) => {
		if (!user) return;
		try {
			const cloudProject =
				project.api ??
				(await cloud.projects.create(user.access_token, {
					name: project.title,
					description: project.description,
					uid: project.id
				}));
			dispatch('updated', { ...project, api: { ...cloudProject, sync: event.detail } });
		} catch (error) {
			console.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	};
</script>

<section class="space-y-2">
	{#if user}
		<h2 class="text-xl">GitButler Cloud</h2>

		<header>
			<span class="text-text-subdued"> Summary generation </span>
		</header>

		<div
			class="flex flex-row items-center justify-between rounded-lg border border-light-400 p-2 dark:border-dark-500"
		>
			<div class="flex flex-col space-x-3">
				<div class="flex flex-row items-center gap-x-1">
					<Checkbox
						name="sync"
						disabled={user === undefined}
						checked={$aiGenEnabled}
						on:change={() => {
							$aiGenEnabled = !$aiGenEnabled;
						}}
					/>
					<label class="ml-2" for="sync">Enable branch and commit message generation.</label>
				</div>
				<div class="pl-4 pr-8 text-sm text-light-700 dark:text-dark-200">
					Uses OpenAI's API. If enabled, diffs will sent to OpenAI's servers when pressing the
					"Generate message" button.
				</div>
			</div>
		</div>
		{#if user.role === 'admin'}
			<header>
				<span class="text-text-subdued"> Full data synchronization </span>
			</header>
			<div
				class="flex flex-row items-center justify-between rounded-lg border border-light-400 p-2 dark:border-dark-500"
			>
				<div class="flex flex-row space-x-3">
					<div class="flex flex-row items-center gap-1">
						<Checkbox
							name="sync"
							disabled={user === undefined}
							checked={project.api?.sync || false}
							on:change={onSyncChange}
						/>
						<label class="ml-2" for="sync">
							Sync my history, repository and branch data for backup, sharing and team features.
						</label>
					</div>
				</div>
			</div>

			{#if project.api}
				<div class="flex flex-row justify-end space-x-2">
					<div class="p-1">
						<Link
							target="_blank"
							rel="noreferrer"
							href="{PUBLIC_API_BASE_URL}projects/{project.api?.repository_id}"
							>Go to GitButler Cloud Project</Link
						>
					</div>
				</div>
			{/if}
		{/if}
	{:else}
		<Login {userService} />
	{/if}
</section>
