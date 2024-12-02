<script lang="ts">
	import { invoke, listen } from '$lib/backend/ipc';
	import * as zip from '$lib/backend/zip';
	import { User } from '$lib/stores/user';
	import * as toasts from '$lib/utils/toasts';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';

	type Feedback = {
		id: number;
		user_id: number;
		feedback: string;
		context: string;
		created_at: string;
		updated_at: string;
	};

	const httpClient = getContext(HttpClient);
	const user = getContextStore(User);

	export function show() {
		modal?.show();
	}

	async function gitIndexLength() {
		return await invoke<void>('git_index_size', {
			projectId: projectId
		});
	}

	let modal: ReturnType<typeof Modal> | undefined = $state();

	let messageInputValue = $state('');
	let emailInputValue = $state('');
	let sendLogs = $state(false);
	let sendProjectRepository = $state(false);

	const projectId = $derived($page.params.projectId!);

	function reset() {
		messageInputValue = '';
		sendLogs = false;
		sendProjectRepository = false;
	}

	async function readZipFile(path: string, filename?: string): Promise<File | Blob> {
		const { readFile } = await import('@tauri-apps/plugin-fs');
		const file = await readFile(path);
		const fileName = filename ?? path.split('/').pop();
		return fileName
			? new File([file], fileName, { type: 'application/zip' })
			: new Blob([file], { type: 'application/zip' });
	}

	async function submit() {
		const message = messageInputValue;
		const email = $user?.email ?? emailInputValue;

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
				sendLogs ? zip.logs().then(async (path) => await readZipFile(path, 'logs.zip')) : undefined,
				sendProjectRepository
					? zip
							.projectData({ projectId })
							.then(async (path) => await readZipFile(path, 'project.zip'))
					: undefined
			]).then(
				async ([logs, repo]) =>
					await createFeedback($user?.access_token, {
						email,
						message,
						context,
						logs,
						repo
					})
			),
			{
				loading: !sendLogs && !sendProjectRepository ? 'Sending feedback...' : 'Uploading data...',
				success: 'Feedback sent successfully',
				error: 'Failed to send feedback'
			}
		);
		close();
	}

	async function createFeedback(
		token: string | undefined,
		params: {
			email?: string;
			message: string;
			context?: string;
			logs?: Blob | File;
			repo?: Blob | File;
		}
	): Promise<Feedback> {
		const formData = new FormData();
		formData.append('message', params.message);
		if (params.email) formData.append('email', params.email);
		if (params.context) formData.append('context', params.context);
		if (params.logs) formData.append('logs', params.logs);
		if (params.repo) formData.append('repo', params.repo);

		// Content Type must be unset for the right form-data border to be set automatically
		return await httpClient.put('feedback', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});
	}

	function close() {
		reset();
		modal?.close();
	}

	onMount(() => {
		const unsubscribe = listen<string>('menu://help/share-debug-info/clicked', () => {
			show();
		});

		return () => {
			unsubscribe();
		};
	});
</script>

<Modal
	bind:this={modal}
	title="Share debug data with GitButler team for review"
	onSubmit={async () => await submit()}
>
	<div class="content-wrapper">
		<p class="content-wrapper__help-text text-13 text-body">
			If you are having trouble, please share your project and logs with the GitButler team. We will
			review it for you and help identify how we can help resolve the issue.
		</p>

		{#if !$user}
			<Textbox
				label="Email"
				placeholder="Provide an email so that we can get back to you"
				type="email"
				bind:value={emailInputValue}
				required
				autocomplete={false}
				autocorrect={false}
				spellcheck
				autofocus
			/>
		{/if}

		<Textarea
			label="Comments"
			placeholder="Provide any steps necessary to reproduce the problem."
			spellcheck
			id="comments"
			minRows={6}
			maxRows={10}
			bind:value={messageInputValue}
		/>

		<div class="content-wrapper__section">
			<span class="text-16 text-semibold"> Share logs </span>
			<span class="content-wrapper__help-text text-13 text-body">
				We personally ensure all information you share with us will be reviewed internally only and
				discarded post-resolution
			</span>
		</div>

		<div class="content-wrapper__checkbox-group">
			<div class="content-wrapper__checkbox">
				<Checkbox name="logs" bind:checked={sendLogs} />
				<label class="text-13" for="logs">Share logs</label>
			</div>

			{#if projectId}
				<div class="content-wrapper__checkbox">
					<Checkbox name="project-repository" bind:checked={sendProjectRepository} />
					<label class="text-13" for="project-repository">Share project repository</label>
				</div>
			{/if}
		</div>
	</div>

	<!-- Use our own close function -->
	{#snippet controls()}
		<Button style="ghost" outline type="reset" onclick={close}>Close</Button>
		<Button style="pop" kind="solid" type="submit">Share with GitButler</Button>
	{/snippet}
</Modal>

<style>
	.content-wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.content-wrapper__section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.content-wrapper__help-text {
		opacity: 0.6;
	}

	.content-wrapper__checkbox-group {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.content-wrapper__checkbox {
		display: flex;
		align-items: center;
		gap: 10px;
	}
</style>
