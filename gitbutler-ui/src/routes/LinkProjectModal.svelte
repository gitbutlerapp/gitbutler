<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { IconFolder, IconLoading } from '$lib/icons';
	import IconFolderPlus from '$lib/icons/IconFolderPlus.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { compareDesc, formatDistanceToNow } from 'date-fns';
	import leven from 'leven';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { Project, ProjectService } from '$lib/backend/projects';
	import type { Observable } from 'rxjs';
	import { goto } from '$app/navigation';

	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let projectService: ProjectService;
	export let user: User | undefined;

	$: projects$ = projectService.projects$;
	$: cloudProjects = user ? cloud.projects.list(user.access_token) : Promise.resolve([]);

	let selectedRepositoryId: string | null = null;

	let project$: Observable<Project | undefined>;

	export async function show(id: string) {
		await cloudProjects;
		if (!user) return;
		project$ = projectService.getProject(id);
		modal.show();
	}

	let modal: Modal;

	let isLinking = false;
	export async function onLinkClicked(project: Project | undefined) {
		isLinking = true;
		const projects = await cloudProjects;

		try {
			if (!project) return;
			const existingCloudProject = projects.find(
				(project) => project.repository_id == selectedRepositoryId
			);
			if (existingCloudProject !== undefined && project) {
				await projectService
					.updateProject({ ...project, api: { ...existingCloudProject, sync: true } })
					.then(() => toasts.success(`Project linked`));
			} else if (selectedRepositoryId == null && user && project && $project$) {
				const cloudProject = await cloud.projects.create(user.access_token, {
					name: $project$.title,
					description: $project$.description,
					uid: $project$.id
				});
				await projectService
					.updateProject({ ...project, api: { ...cloudProject, sync: true } })
					.then(() => toasts.success(`Project linked`));
				goto(`/${$project$.id}/`);
			}
			modal.close();
		} catch (e) {
			toasts.error(`Failed to link project`);
		} finally {
			isLinking = false;
		}
	}
</script>

<Modal bind:this={modal} title="GitButler Cloud">
	{#await Promise.all([cloudProjects])}
		<IconLoading class="m-auto animate-spin" />
	{:then}
		<div class="-mt-4 flex flex-auto pt-4">
			<ul class="min-w-1/2 flex flex-col gap-2 pr-10 pt-4">
				<p>Connect to GitButler Cloud to enable Cloud features such as:</p>

				<li>
					<h4 class="font-semibold">✨ AI generated commit messages</h4>
					<p class="p-1">
						Instead of writing commit messages yourself, let GitButler do it for you.
					</p>
				</li>

				<li>
					<h4 class="font-semibold">🤖 AI hunk summarization</h4>
					<p class="p-1">
						GitButler will display a short summary of the changed you've made for an easier
						overview.
					</p>
				</li>

				<li>
					<h4 class="font-semibold">☁️ Clients Syncronization</h4>
					<p class="p-1">All your projects will be synced across all your devices.</p>
				</li>

				<li>
					<h4 class="font-semibold">🗓️ More to come...</h4>
				</li>
			</ul>

			{#await cloudProjects}
				loading...
			{:then projects}
				{#if projects.length !== 0}
					<div class="-mb-4 -mr-4 -mt-4 flex w-full flex-col gap-2 bg-[#000000]/20 pb-6 pt-6">
						<ul class="flex flex-auto flex-col gap-2 overflow-y-scroll px-4 pb-4">
							<button
								class="flex w-full items-start gap-[10px] rounded border bg-light-50 p-2 text-left shadow-sm transition-colors duration-200 hover:cursor-pointer hover:border dark:bg-dark-800"
								class:border-blue-400={selectedRepositoryId === null}
								class:border-transparent={selectedRepositoryId !== null}
								on:click={() => (selectedRepositoryId = null)}
							>
								<IconFolderPlus class="text-blue-500" />
								<div class="flex flex-col">
									<span>Create new project</span>
									<span class="text-xs text-light-700 dark:text-dark-300">
										Syncing will begin after first save
									</span>
								</div>
							</button>
							{#each projects
								// filter out projects that are already linked
								.map( (project) => ({ ...project, disabled: $projects$?.some((p) => p?.api?.repository_id === project.repository_id) }) )
								// sort by last updated
								.sort((a, b) => compareDesc(new Date(a.updated_at), new Date(b.updated_at)))
								// sort by name
								.sort((a, b) => a.name.localeCompare(b.name))
								// sort by name distance to linking project title
								.sort( (a, b) => (!$project$ ? 0 : leven(a.name.toLowerCase(), $project$.title.toLowerCase()) < leven(b.name.toLowerCase(), $project$.title.toLowerCase()) ? -1 : 1) )
								// disbled on the bottom
								.sort((a, b) => (a.disabled === b.disabled ? 0 : a.disabled ? 1 : -1)) as project}
								<button
									disabled={project.disabled}
									class="flex w-full items-start gap-[10px] rounded border bg-light-50 p-2 text-left shadow-sm transition-colors duration-200 hover:cursor-pointer hover:border dark:bg-dark-800"
									class:opacity-40={project.disabled}
									class:border-blue-400={selectedRepositoryId === project.repository_id}
									class:border-transparent={selectedRepositoryId !== project.repository_id}
									on:click={() => (selectedRepositoryId = project.repository_id)}
								>
									<IconFolder class="text-blue-500" />
									<div class="flex flex-col">
										<span>{project.name}</span>
										<span class="text-xs text-light-700 dark:text-dark-300">
											Last updated: {formatDistanceToNow(new Date(project.updated_at))} ago
										</span>
									</div>
								</button>
							{/each}
						</ul>
					</div>
				{/if}
			{/await}
		</div>
	{/await}

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Not Now</Button>
		<Button color="primary" loading={isLinking} on:click={() => onLinkClicked($project$)}>
			{#if selectedRepositoryId === null}
				Connect
			{:else}
				Link
			{/if}
		</Button>
	</svelte:fragment>
</Modal>
