<script lang="ts">
	import * as toasts from '$lib/utils/toasts';
	import { userStore } from '$lib/stores/user';
	import { goto } from '$app/navigation';
	import CloudForm from './CloudForm.svelte';
	import DetailsForm from './DetailsForm.svelte';
	import KeysForm from './KeysForm.svelte';
	import * as projects from '$lib/backend/projects';
	import { updateProject } from '$lib/backend/projects';
	import type { PageData } from './$types';
	import BackButton from '$lib/components/BackButton.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Button from '$lib/components/Button.svelte';

	export let data: PageData;
	const { project, cloud } = data;
	const user = userStore;

	let deleteConfirmationModal: Modal;
	let isDeleting = false;

	const onDeleteClicked = () =>
		Promise.resolve()
			.then(() => (isDeleting = true))
			.then(() => projects.deleteProject($project?.id))
			.then(() => deleteConfirmationModal.close())
			.catch((e) => {
				console.error(e);
				toasts.error('Failed to delete project');
			})
			.then(() => goto('/'))
			.then(() => toasts.success('Project deleted'))
			.finally(() => (isDeleting = false));

	const onKeysUpdated = (e: { detail: { preferred_key: projects.Key } }) =>
		updateProject({ ...$project, ...e.detail });
	const onCloudUpdated = (e: { detail: projects.Project }) =>
		updateProject({ ...$project, ...e.detail });
	const onDetailsUpdated = async (e: { detail: projects.Project }) => {
		const api =
			$user && e.detail.api
				? await cloud.projects.update($user?.access_token, e.detail.api.repository_id, {
						name: e.detail.title,
						description: e.detail.description
				  })
				: undefined;

		updateProject({
			...e.detail,
			api: api ? { ...api, sync: e.detail.api?.sync || false } : undefined
		});
	};
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none">
	<div class="mx-auto flex min-w-min max-w-xl flex-col gap-y-6 overflow-visible p-8">
		<div class="flex w-full">
			<BackButton />
			<h2 class="text-2xl font-medium">Project settings</h2>
		</div>
		<div class="bg-color-1 h-[0.0625rem] shrink-0" />
		<CloudForm project={$project} on:updated={onCloudUpdated} />
		<div class="bg-color-1 h-[0.0625rem] shrink-0" />
		<DetailsForm project={$project} on:updated={onDetailsUpdated} />
		<div class="bg-color-1 h-[0.0625rem] shrink-0" />
		<KeysForm project={$project} on:updated={onKeysUpdated} />

		<div class="bg-color-1 h-[0.0625rem] shrink-0" />
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

		<div class="bg-color-1 h-[0.0625rem] shrink-0" />
		<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
			Delete project
		</Button>
	</div>
</div>

<Modal bind:this={deleteConfirmationModal} title="Delete {$project?.title}?">
	<p>
		Are you sure you want to delete
		<span class="font-bold">{$project?.title}</span>? This can’t be undone.
	</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>
			Delete project
		</Button>
	</svelte:fragment>
</Modal>
