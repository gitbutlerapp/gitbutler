<script lang="ts">
	import { Button, Modal } from '$lib/components';
	import { toasts, api, stores } from '$lib';
	import { goto } from '$app/navigation';
	import CloudForm from './CloudForm.svelte';
	import DetailsForm from './DetailsForm.svelte';
	import type { Project } from '$lib/api';
	import type { PageData } from './$types';

	export let data: PageData;
	const { projects, project, cloud } = data;

	console.log($project);
	const user = stores.user;

	let deleteConfirmationModal: Modal;
	let isDeleting = false;

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => api.projects.del({ id: $project?.id }))
			.then(() => deleteConfirmationModal.close())
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => goto('/'))
			.then(() => projects.update((projects) => projects.filter((p) => p.id !== $project?.id)))
			.then(() => toasts.success('Project deleted'))
			.finally(() => (isDeleting = false));

	const onCloudUpdated = (e: { detail: Project }) => project.update({ ...e.detail });
	const onDetailsUpdated = async (e: { detail: Project }) => {
		const api =
			$user && e.detail.api
				? await cloud.projects.update($user?.access_token, e.detail.api.repository_id, {
						name: e.detail.title,
						description: e.detail.description
				  })
				: undefined;

		project.update({
			...e.detail,
			api: api ? { ...api, sync: e.detail.api?.sync || false } : undefined
		});
	};
</script>

<div class="mx-auto h-fit w-full max-w-xl bg-light-200 py-10 dark:bg-dark-900">
	<div class="flex flex-col gap-y-8">
		<div class="flex w-full justify-between">
			<h2 class="text-2xl font-medium">Project settings</h2>
		</div>
		<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />
		<CloudForm project={$project} on:updated={onCloudUpdated} />
		<DetailsForm project={$project} on:updated={onDetailsUpdated} />

		<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />
		<div class="flex gap-x-4">
			<a
				href="https://discord.gg/wDKZCPEjXC"
				target="_blank"
				rel="noreferrer"
				class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
			>
				<p class="mb-2 font-medium">Join our Discorder</p>
				<p class="text-light-700 dark:text-dark-200">
					Join our community and share feedback, requests, or ask a question.
				</p>
			</a>
			<a
				href="mailto:hello@gitbutler.com?subject=Feedback or question!"
				target="_blank"
				class="flex-1 rounded border border-light-200 bg-white p-4 dark:border-dark-400 dark:bg-dark-700"
			>
				<p class="mb-2 font-medium">Contact us</p>
				<p class="text-light-700 dark:text-dark-200">
					If you have an issue or any questions, contact us.
				</p>
			</a>
		</div>

		<div class="h-[0.0625rem] bg-light-400 dark:bg-dark-700" />
		<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
			Delete project
		</Button>
	</div>
</div>

<Modal bind:this={deleteConfirmationModal} title="Delete {$project?.title}?">
	<p>
		Are you sure you want to delete the project,
		<span class="font-bold text-white">{$project?.title}</span>? This can’t be undone.
	</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>
			Delete project
		</Button>
	</svelte:fragment>
</Modal>
