<script lang="ts">
	import { toasts } from '$lib';
	import * as zip from '$lib/api/ipc/zip';
	import { Button, Checkbox, Modal } from '$lib/components';
	import { page } from '$app/stores';
	import type { User, getCloudApiClient } from '$lib/api/cloud/api';

	export let user: User | null;
	export let cloud: ReturnType<typeof getCloudApiClient>;

	export function show() {
		modal.show();
	}

	let modal: Modal;

	let messageInputValue = '';
	let emailInputValue = '';
	let sendLogs = false;
	let sendProjectData = false;
	let sendProjectRepository = false;

	$: projectId = $page.params.projectId;

	const reset = () => {
		messageInputValue = '';
		sendLogs = false;
		sendProjectData = false;
		sendProjectRepository = false;
	};

	const readZipFile = (path: string, filename?: string): Promise<File | Blob> =>
		import('@tauri-apps/api/fs').then(async ({ readBinaryFile }) => {
			const file = await readBinaryFile(path);
			const fileName = filename ?? path.split('/').pop();
			return fileName
				? new File([file], fileName, { type: 'application/zip' })
				: new Blob([file], { type: 'application/zip' });
		});

	const onSubmit = () => {
		const message = messageInputValue;
		const email = user?.email ?? emailInputValue;
		toasts.promise(
			Promise.all([
				sendLogs ? zip.logs().then((path) => readZipFile(path, 'logs.zip')) : undefined,
				sendProjectData
					? zip.gitbutlerData({ projectId }).then((path) => readZipFile(path, 'data.zip'))
					: undefined,
				sendProjectRepository
					? zip.projectData({ projectId }).then((path) => readZipFile(path, 'project.zip'))
					: undefined
			]).then(async ([logs, data, repo]) =>
				cloud.feedback.create(user?.access_token, {
					email,
					message,
					logs,
					data,
					repo
				})
			),
			{
				loading:
					!sendLogs && !sendProjectData && !sendProjectRepository
						? 'Sending feedback...'
						: 'Uploading data...',
				success: 'Feedback sent successfully',
				error: 'Failed to send feedback'
			}
		);
		onClose();
	};

	const onClose = () => {
		reset();
		modal.close();
	};
</script>

<Modal bind:this={modal} title="Share with GitButler team for review" on:close={onClose}>
	<div class="flex flex-col gap-4">
		<p>
			If you are having trouble, please share your project and logs with the Gitbutler team. We will
			review it for you and help identify how we can help resolve the issue.
		</p>

		{#if !user}
			<div class="flex flex-col gap-1">
				<label for="email">Email</label>
				<input
					name="email"
					placeholder="Provide an email if you want to hear back from us"
					type="email"
					bind:value={emailInputValue}
				/>
			</div>
		{/if}

		<div class="flex flex-col gap-1">
			<label for="comments" class="text-xl font-semibold">Comments</label>

			<textarea
				placeholder="Provide any steps necessary to reproduce the problem."
				autocomplete="off"
				autocorrect="off"
				spellcheck="true"
				name="comments"
				rows="6"
				class="h-full w-full resize-none"
				bind:value={messageInputValue}
			/>
		</div>

		<div class="flex flex-col gap-1">
			<span class="text-xl font-semibold"> Share logs </span>
			<span class="text-sm text-text-subdued">
				We personally ensure all information you share with us will be reviewed internally only and
				discarded post-resolution
			</span>
		</div>

		<div class="flex flex-col gap-3">
			<div class="flex items-center gap-2">
				<Checkbox name="logs" bind:checked={sendLogs} />
				<label for="logs">Share logs</label>
			</div>

			{#if projectId}
				<div class="flex items-center gap-2">
					<Checkbox name="project-data" bind:checked={sendProjectData} />
					<label for="project-data">Share project data</label>
				</div>

				<div class="flex items-center gap-2">
					<Checkbox name="project-repository" bind:checked={sendProjectRepository} />
					<label for="project-data">Share project repository</label>
				</div>
			{/if}
		</div>
	</div>

	<svelte:fragment slot="controls">
		<Button kind="outlined" on:click={onClose}>Close</Button>
		<Button color="purple" on:click={onSubmit}>Share with GitButler</Button>
	</svelte:fragment>
</Modal>
