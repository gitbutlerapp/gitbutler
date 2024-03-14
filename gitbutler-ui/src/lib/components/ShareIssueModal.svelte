<script lang="ts">
	import TextArea from './TextArea.svelte';
	import { invoke } from '$lib/backend/ipc';
	import * as zip from '$lib/backend/zip';
	import Button from '$lib/components/Button.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { getVersion } from '@tauri-apps/api/app';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import { page } from '$app/stores';

	export let user: User | undefined;
	export let cloud: ReturnType<typeof getCloudApiClient>;

	export function show() {
		modal.show();
	}

	function gitIndexLength() {
		return invoke<void>('git_index_size', {
			projectId: projectId
		});
	}

	let modal: Modal;

	let messageInputValue = '';
	let emailInputValue = '';
	let sendLogs = false;
	let sendProjectData = false;
	let sendProjectRepository = false;

	$: projectId = $page.params.projectId;

	function reset() {
		messageInputValue = '';
		sendLogs = false;
		sendProjectData = false;
		sendProjectRepository = false;
	}

	async function readZipFile(path: string, filename?: string): Promise<File | Blob> {
		const { readBinaryFile } = await import('@tauri-apps/api/fs');
		const file = await readBinaryFile(path);
		const fileName = filename ?? path.split('/').pop();
		return fileName
			? new File([file], fileName, { type: 'application/zip' })
			: new Blob([file], { type: 'application/zip' });
	}

	async function onSubmit() {
		const message = messageInputValue;
		const email = user?.email ?? emailInputValue;

		// put together context information to send with the feedback
		let context = '';
		const appVersion = await getVersion();
		const indexLength = await gitIndexLength();
		context += 'GitButler Version: ' + appVersion + '\n';
		context += 'Browser: ' + navigator.userAgent + '\n';
		context += 'URL: ' + window.location.href + '\n';
		context += 'Length of index: ' + indexLength + '\n';

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
					context,
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
	}

	function onClose() {
		reset();
		modal.close();
	}
</script>

<Modal bind:this={modal} on:close={onClose} title="Share debug data with GitButler team for review">
	<div class="flex flex-col gap-4">
		<p class="text-color-3">
			If you are having trouble, please share your project and logs with the GitButler team. We will
			review it for you and help identify how we can help resolve the issue.
		</p>

		{#if !user}
			<div class="flex flex-col gap-1">
				<label for="email">Email</label>
				<input
					name="email"
					placeholder="Provide an email so that we can get back to you"
					type="email"
					class="input"
					bind:value={emailInputValue}
					required
					autocomplete="off"
					autocorrect="off"
					spellcheck="true"
				/>
			</div>
		{/if}

		<div class="flex flex-col gap-1">
			<label for="comments">Comments</label>

			<TextArea
				placeholder="Provide any steps necessary to reproduce the problem."
				autocomplete="off"
				autocorrect="off"
				spellcheck
				id="comments"
				rows={6}
				bind:value={messageInputValue}
			/>
		</div>

		<div class="flex flex-col gap-1">
			<span class="text-xl font-semibold"> Share logs </span>
			<span class="text-color-3 text-sm">
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
		<Button color="primary" on:click={onSubmit}>Share with GitButler</Button>
	</svelte:fragment>
</Modal>
