<script lang="ts">
	import { toasts, api } from '$lib';
	import { Button, Modal } from '$lib/components';
	import { page } from '$app/stores';
	import type { User } from '$lib/api';

	export let user: User;
	export let cloud: ReturnType<typeof api.CloudApi>;

	export const show = () => modal.show();

	let modal: Modal;

	let comments = '';
	let sendLogs = true;
	let sendProjectData = true;
	let sendProjectRepository = true;

	let isSending = false;

	$: projectId = $page.params.projectId;

	const reset = () => {
		comments = '';
		sendLogs = true;
		sendProjectData = true;
		sendProjectRepository = true;
		isSending = false;
	};

	const readZipFile = (path: string): Promise<Blob> =>
		import('@tauri-apps/api/fs').then(async ({ readBinaryFile }) => {
			const file = await readBinaryFile(path);
			return new Blob([file], { type: 'application/zip' });
		});

	const onSubmit = () =>
		Promise.resolve()
			.then(() => (isSending = true))
			.then(() =>
				Promise.all([
					sendLogs ? api.zip.logs().then(readZipFile) : undefined,
					sendProjectData ? api.zip.gitbutlerData({ projectId }).then(readZipFile) : undefined,
					sendProjectRepository ? api.zip.projectData({ projectId }).then(readZipFile) : undefined
				])
			)
			.then(async ([logs, data, repo]) =>
				cloud.feedback.create(user?.access_token, {
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
			Submit an issue to be review by the GitButler team. This information is collected anonymously
			and all shared data will only be used internal and deleted after resolution is found.
		</p>

		<div class="flex flex-col gap-1">
			<span>Comments</span>

			<textarea
				placeholder="Provide any steps nessesary to reproduce the problem."
				autocomplete="off"
				autocorrect="off"
				spellcheck="true"
				name="comments"
				disabled={isSending}
				rows="6"
				class="h-full w-full resize-none rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100  hover:border-zinc-500/80 focus:border-[] focus:focus:border-blue-600 focus:ring-2 focus:ring-blue-600/30"
				bind:value={comments}
			/>
		</div>

		<div class="flex flex-col gap-1">
			<span class="text-xl font-semibold"> Share logs </span>
			<span class="text-sm text-text-subdued">
				Sharing will upload corresponding files with GitButler team. All share files will be deleted
				after problem is removed.
			</span>
		</div>

		<div class="flex flex-col gap-3 text-lg">
			<div class="flex items-center gap-2">
				<input
					type="checkbox"
					disabled={isSending}
					checked
					bind:value={sendLogs}
					name="logs"
					id="logs"
					class="h-4 w-4"
				/>
				<label for="logs">Share logs</label>
			</div>

			{#if projectId}
				<div class="flex items-center gap-2">
					<input
						type="checkbox"
						disabled={isSending}
						checked
						bind:value={sendProjectData}
						name="project-data"
						id="project-data"
						class="h-4 w-4"
					/>
					<label for="project-data">Share project data</label>
				</div>

				<div class="flex items-center gap-2">
					<input
						type="checkbox"
						checked
						disabled={isSending}
						bind:value={sendProjectRepository}
						name="project-repository"
						id="project-repository"
						class="h-4 w-4"
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
