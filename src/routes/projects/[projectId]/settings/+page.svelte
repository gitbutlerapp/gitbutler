<script lang="ts">
	import { Button, Modal } from '$lib/components';
	import type { PageData } from './$types';
	import { open } from '@tauri-apps/api/shell';
	import { toasts, api, stores, events } from '$lib';
	import { goto } from '$app/navigation';
	import CloudForm from './CloudForm.svelte';
	import DetailsForm from './DetailsForm.svelte';
	import type { Project } from '$lib/api';

	export let data: PageData;
	const { projects, project, cloud } = data;

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

<div class="mx-auto h-full overflow-auto p-4">
	<div class="mx-auto max-w-2xl p-4">
		<div class="flex flex-col space-y-6">
			<div class="space-y-0">
				<div class="text-2xl font-medium">Project Settings</div>
				<div class="">
					How shall I manage your project settings for <strong>{$project?.title}</strong>?
				</div>
			</div>
			<hr class="border-zinc-600" />
			{#await project.load() then}
				<CloudForm project={$project} on:updated={onCloudUpdated} />
				<DetailsForm project={$project} on:updated={onDetailsUpdated} />
			{/await}

			<hr class="border-zinc-600" />
			<div class="flex flex-col gap-1">
				<h2 class="text-xl">Need help?</h2>
				<div class="grid grid-cols-3 gap-4">
					<button
						class="flex flex-col gap-2 rounded border border-zinc-700 bg-card-default p-3 text-left text-zinc-400 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
						on:click={() => events.emit('openSendIssueModal')}
					>
						<h2 class="text-lg text-zinc-300">Having troubles?</h2>
						<div class="text-zinc-500">
							Are having issues? Contact us, privately share files, and logs.
						</div>
					</button>

					<button
						class="flex flex-col gap-2 rounded border border-zinc-700 bg-card-default p-3 text-left text-zinc-400 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
						on:click={() => open('https://discord.gg/wDKZCPEjXC')}
					>
						<h2 class="text-lg text-zinc-300">Join our Discord</h2>
						<div class="text-zinc-500">
							Join our community and share feedback, requests, or ask a question.
						</div>
					</button>

					<button
						class="flex flex-col gap-2 rounded border border-zinc-700 bg-card-default p-3 text-left text-zinc-400 shadow transition duration-150 ease-out hover:bg-card-active hover:ease-in"
						on:click={() => open('mailto:hello@gitbutler.com?subject=Feedback or question!')}
					>
						<h2 class="text-lg text-zinc-300">Get Support</h2>
						<div class="text-zinc-500">If you have an issue or any questions, contact us.</div>
					</button>
				</div>
			</div>

			<hr class="border-zinc-600" />
			<Button color="destructive" kind="outlined" on:click={() => deleteConfirmationModal.show()}>
				Delete project
			</Button>
		</div>
	</div>
</div>

<Modal bind:this={deleteConfirmationModal} title="Delete {$project.title}?">
	<p>
		Are you sure you want to delete the project,
		<span class="font-bold text-white">{$project.title}</span>? This can’t be undone.
	</p>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="destructive" loading={isDeleting} on:click={onDeleteClicked}>
			Delete project
		</Button>
	</svelte:fragment>
</Modal>
