<script lang="ts">
	import { page } from '$app/stores';
	import { BACKEND } from '$lib/backend';
	import { FILE_SERVICE } from '$lib/files/fileService';
	import { GIT_SERVICE } from '$lib/git/gitService';
	import { SHORTCUT_SERVICE } from '$lib/shortcuts/shortcutService';
	import { DATA_SHARING_SERVICE } from '$lib/support/dataSharing';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/core/context';
	import { HTTP_CLIENT } from '@gitbutler/shared/network/httpClient';

	import { Button, Checkbox, Modal, Textarea, EmailTextbox, chipToasts } from '@gitbutler/ui';

	type Feedback = {
		id: number;
		user_id: number;
		feedback: string;
		context: string;
		created_at: string;
		updated_at: string;
	};

	const shortcutService = inject(SHORTCUT_SERVICE);
	const httpClient = inject(HTTP_CLIENT);
	const fileService = inject(FILE_SERVICE);
	const dataSharingService = inject(DATA_SHARING_SERVICE);
	const gitService = inject(GIT_SERVICE);
	const user = inject(USER);
	const backend = inject(BACKEND);

	export function show() {
		modal?.show();
	}

	async function gitIndexLength() {
		return await gitService.indexSize(projectId);
	}

	let modal: ReturnType<typeof Modal> | undefined = $state();

	let messageInputValue = $state('');
	let emailInputValue = $state('');
	let sendLogs = $state(false);
	let sendProjectRepository = $state(false);
	let sendGraph = $state(false);

	const projectId = $derived($page.params.projectId!);

	function reset() {
		messageInputValue = '';
		sendLogs = false;
		sendProjectRepository = false;
		sendGraph = false;
	}

	async function readZipFile(path: string, filename?: string): Promise<File | Blob> {
		// Using `new Uint8Array` to get an `ArrayBuffer` rather than `ArrayBufferLike`.
		const file = new Uint8Array(await fileService.readFile(path));
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
		const appInfo = await backend.getAppInfo();
		const appVersion = appInfo.version;
		const indexLength = await gitIndexLength();
		context += 'GitButler Version: ' + appVersion + '\n';
		context += 'Browser: ' + navigator.userAgent + '\n';
		context += 'URL: ' + window.location.href + '\n';
		context += 'Length of index: ' + indexLength + '\n';

		chipToasts.promise(
			Promise.all([
				sendLogs
					? dataSharingService.logs().then(async (path) => await readZipFile(path, 'logs.zip'))
					: undefined,
				sendProjectRepository
					? dataSharingService
							.projectData(projectId)
							.then(async (path) => await readZipFile(path, 'project.zip'))
					: undefined,
				sendGraph
					? dataSharingService
							.graphFile(projectId)
							.then(async (path) => await readZipFile(path, 'graph.zip'))
					: undefined
			]).then(
				async ([logs, repo, graph]) =>
					await createFeedback($user?.access_token, {
						email,
						message,
						context,
						logs,
						repo,
						graph
					})
			),
			{
				loading:
					!sendLogs && !sendProjectRepository && !sendGraph
						? 'Sending feedback...'
						: 'Uploading data...',
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
			graph?: Blob | File;
		}
	): Promise<Feedback> {
		const formData = new FormData();
		formData.append('message', params.message);
		if (params.email) formData.append('email', params.email);
		if (params.context) formData.append('context', params.context);
		if (params.logs) formData.append('logs', params.logs);
		if (params.repo) formData.append('repo', params.repo);
		if (params.graph) formData.append('graph', params.graph);

		// Content Type must be unset for the right form-data border to be set automatically
		return await httpClient.put('feedback', {
			body: formData,
			headers: { 'Content-Type': undefined }
		});
	}

	async function close() {
		await modal?.close();
		reset();
	}

	$effect(() => shortcutService.on('share-debug-info', () => show()));
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
			<EmailTextbox
				label="Email"
				placeholder="Provide an email so that we can get back to you"
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
				<div class="content-wrapper__checkbox">
					<Checkbox name="graph" bind:checked={sendGraph} />
					<label class="text-13" for="graph">Share anonymized commit-graph</label>
				</div>
			{/if}
		</div>
	</div>

	<!-- Use our own close function -->
	{#snippet controls()}
		<Button kind="outline" type="reset" onclick={close}>Close</Button>
		<Button disabled={!sendLogs && !sendProjectRepository && !sendGraph} style="pop" type="submit"
			>Share with GitButler</Button
		>
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
