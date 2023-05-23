<script lang="ts">
	import { toasts, api } from '$lib';
	import { Button, Checkbox, Modal } from '$lib/components';
	import { page } from '$app/stores';
	import type { User } from '$lib/api';

	export let user: User | null;
	export let cloud: ReturnType<typeof api.CloudApi>;

	export const show = () => modal.show();

	let modal: Modal;

	let comments = '';
	let sendLogs = false;
	let sendProjectData = false;
	let sendProjectRepository = false;
	let email = '';

	let isSending = false;

	$: projectId = $page.params.projectId;

	const reset = () => {
		comments = '';
		sendLogs = true;
		sendProjectData = true;
		sendProjectRepository = true;
		isSending = false;
	};

	const readZipFile = (path: string, filename?: string): Promise<File | Blob> =>
		import('@tauri-apps/api/fs').then(async ({ readBinaryFile }) => {
			const file = await readBinaryFile(path);
			const fileName = filename ?? path.split('/').pop();
			return fileName
				? new File([file], fileName, { type: 'application/zip' })
				: new Blob([file], { type: 'application/zip' });
		});

	const onSubmit = () =>
		Promise.resolve()
			.then(() => (isSending = true))
			.then(() =>
				Promise.all([
					sendLogs ? api.zip.logs().then((path) => readZipFile(path, 'logs.zip')) : undefined,
					sendProjectData
						? api.zip.gitbutlerData({ projectId }).then((path) => readZipFile(path, 'data.zip'))
						: undefined,
					sendProjectRepository
						? api.zip.projectData({ projectId }).then((path) => readZipFile(path, 'project.zip'))
						: undefined
				])
			)
			.then(async ([logs, data, repo]) =>
				cloud.feedback.create(user?.access_token, {
					email: user?.email ?? email,
					message: comments,
					logs,
					data,
					repo
				})
			)
			.then(() => {
				onClose();
				toasts.success('Issue sent');
			})
			.catch(() => {
				isSending = false;
				toasts.error('Failed to send issue');
			});

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
					bind:value={email}
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
				disabled={isSending}
				rows="6"
				class="h-full w-full resize-none"
				bind:value={comments}
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
				<Checkbox name="logs" bind:checked={sendLogs} disabled={isSending} />
				<label for="logs">Share logs</label>
			</div>

			{#if projectId}
				<div class="flex items-center gap-2">
					<Checkbox name="project-data" bind:checked={sendProjectData} disabled={isSending} />
					<label for="project-data">Share project data</label>
				</div>

				<div class="flex items-center gap-2">
					<Checkbox
						name="project-repository"
						bind:checked={sendProjectRepository}
						disabled={isSending}
					/>
					<label for="project-data">Share project repository</label>
				</div>
			{/if}
		</div>
	</div>

	<svelte:fragment slot="controls">
		<Button kind="outlined" on:click={onClose}>Close</Button>
		<Button color="purple" loading={isSending} on:click={onSubmit}>Share with GitButler</Button>
	</svelte:fragment>
</Modal>
