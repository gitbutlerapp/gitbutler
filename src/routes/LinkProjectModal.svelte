<script lang="ts">
	import leven from 'leven';
	import { Button, Modal } from '$lib/components';
	import { asyncDerived } from '@square/svelte-store';
	import { compareDesc, formatDistanceToNow } from 'date-fns';
	import { IconFolder, IconLoading } from '$lib/icons';
	import { toasts, api, stores } from '$lib';

	export let projects: ReturnType<typeof api.projects.Projects>;
	export let cloud: ReturnType<typeof api.CloudApi>;

	const user = stores.user;

	const cloudProjects = asyncDerived(user, async (user) =>
		user ? await cloud.projects.list(user.access_token) : []
	);

	let selectedRepositoryId: string | null = null;

	let project: ReturnType<typeof api.projects.Project> | undefined;

	export const show = async (id: string) => {
		project = api.projects.Project({ id });
		modal.show();
	};

	let modal: Modal;

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
				modal.close();
			})

			.catch(() => toasts.error(`Failed to link project`))
			.finally(() => (isLinking = false));
</script>

<Modal bind:this={modal} title="Sync with existing GitButler project">
	<div class="-mt-4 flex flex-auto grid-cols-2 pt-4">
		<div class="flex w-1/2 flex-col gap-2 pr-10 pt-4">
			<h3 class="text-lg font-medium">GitButler Cloud projects</h3>
			<p>Syncing projects will save working directory to GitButler Cloud.</p>
			<p>Would you like to link this project to any existing GitButler Cloud projects?</p>
		</div>

		<div class="-mt-4 -mr-4 -mb-4 flex w-1/2 flex-auto flex-col gap-2 bg-[#000000]/20 pt-4">
			<h3 class="px-4 text-lg font-semibold">Existing GitButler Projects</h3>
			{#await Promise.all([cloudProjects.load(), projects.load(), project?.load()])}
				<IconLoading class="m-auto animate-spin" />
			{:then}
				<ul class="flex flex-auto flex-col gap-2 overflow-y-scroll px-4 pb-4">
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
</Modal>
