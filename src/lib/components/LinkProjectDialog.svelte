<script lang="ts">
	import leven from 'leven';
	import { Button, Dialog } from '$lib/components';
	import { asyncDerived } from '@square/svelte-store';
	import { compareDesc, formatDistanceToNow } from 'date-fns';
	import { IconBookmark, IconFolder, IconLoading } from './icons';
	import { toasts, api } from '$lib';

	type Unpromisify<T> = T extends Promise<infer U> ? U : T;

	export let user: ReturnType<typeof api.users.CurrentUser>;
	export let projects: ReturnType<typeof api.projects.Projects>;
	export let cloud: ReturnType<typeof api.CloudApi>;

	const cloudProjects = asyncDerived(user, async (user) =>
		user ? await cloud.projects.list(user.access_token) : []
	);

	let selectedRepositoryId: string | null = null;

	let project: Unpromisify<ReturnType<ReturnType<typeof api.projects.Projects>['get']>> | undefined;

	export const show = async (projectId: string) => {
		project = await projects.get(projectId);
		dialog.show();
	};

	let dialog: Dialog;

	let isLinking = false;
	const onLinkClicked = () =>
		Promise.resolve((isLinking = true))
			.then(async () => {
				const cloudProject = $cloudProjects.find(
					(project) => project.repository_id === selectedRepositoryId
				);
				if (cloudProject !== undefined)
					await project
						?.update({ api: { ...cloudProject, sync: true } })
						.then(() => toasts.success(`Project linked`));
				dialog.close();
			})

			.catch(() => toasts.error(`Failed to link project`))
			.finally(() => (isLinking = false));
</script>

<Dialog bind:this={dialog}>
	<svelte:fragment slot="title">
		<div class="flex items-center gap-3">
			<IconBookmark />
			<span class="text-xl text-zinc-300">Link to existing GitButler project </span>
		</div>
	</svelte:fragment>

	<div class="-m-4 grid h-[296px] w-[620px] flex-auto grid-cols-2">
		<div class="flex flex-col gap-2 px-4 py-6">
			<h3 class="text-lg">Content</h3>

			<p>
				Lorem ipsum dor sit all met. Lorem ipsum dor sit all met. Lorem ipsum dor sit all met. Lorem
				ipsum dor sit all met. Lorem ipsum dor sit all met. Lorem ipsum dor sit all met. Lorem ipsum
				dor sit all met. Lorem ipsum dor sit all met.
			</p>
		</div>

		<div class="flex flex-auto flex-col gap-2 overflow-y-auto bg-[#000000]/20 py-6">
			<h3 class="px-4 text-lg font-semibold">Existing GitButler Projects</h3>
			{#await Promise.all([cloudProjects.load(), projects.load(), project?.load()])}
				<IconLoading class="m-auto animate-spin" />
			{:then}
				<ul class="flex flex-col gap-2 overflow-y-scroll px-4">
					{#each $cloudProjects
						// filter out projects that are already linked
						.filter((project) => $projects?.find((p) => p?.api?.repository_id === project.repository_id) === undefined)
						// sort by last updated
						.sort((a, b) => compareDesc(new Date(a.updated_at), new Date(b.updated_at)))
						// sort by name
						.sort((a, b) => a.name.localeCompare(b.name))
						// sort by name distance to linking project title
						.sort( (a, b) => (!$project ? 0 : leven(a.name.toLowerCase(), $project.title.toLowerCase()) < leven(b.name.toLowerCase(), $project.title.toLowerCase()) ? -1 : 1) ) as project}
						<button
							class="hover:bg-card-hover flex gap-[10px] rounded bg-card-default p-2 text-left shadow-sm transition-colors duration-200 hover:cursor-pointer"
							class:bg-card-active={selectedRepositoryId === project.repository_id}
							on:click={() => (selectedRepositoryId = project.repository_id)}
						>
							<IconFolder class="text-blue-500" />
							<div class="flex flex-col gap-1">
								<span class="text-text-default">{project.name}</span>
								<span class="text-xs text-text-subdued">
									Last updated: {formatDistanceToNow(new Date(project.updated_at))} ago
								</span>
							</div>
						</button>
					{/each}
				</ul>
			{/await}
		</div>
	</div>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Not Now</Button>
		<Button
			disabled={selectedRepositoryId === null}
			color="primary"
			loading={isLinking}
			on:click={onLinkClicked}
		>
			Link projects
		</Button>
	</svelte:fragment>
</Dialog>
