<script lang="ts">
	import { toasts } from '$lib';
	import { Status, type Project, git } from '$lib/api';
	import type { CloudApi, User } from '$lib/api';
	import { Button, Modal, Link } from '$lib/components';
	import { IconGitBranch } from './icons';

	export const show = () => modal.show();

	export let project: Project;
	export let head: string;
	export let statuses: Record<string, Status>;
	export let diffs: Record<string, string>;
	export let user: User;
	export let api: ReturnType<typeof CloudApi>;

	let summary = '';
	let description = '';
	let isAutowriting = false;
	let isCommitting = false;

	const stageAll = async () => {
		const paths = Object.entries(statuses)
			.filter((entry) => Status.isUnstaged(entry[1]))
			.map(([path]) => path);
		if (paths.length === 0) return;
		await git.stage({
			projectId: project.id,
			paths
		});
	};

	const reset = () => {
		summary = '';
		description = '';
	};

	const onCommit = async (e: SubmitEvent) => {
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);
		const summary = formData.get('commit-message') as string;
		const description = formData.get('commit-description') as string;

		isCommitting = true;
		await stageAll();
		git
			.commit({
				projectId: project.id,
				message: description.length > 0 ? `${summary}\n\n${description}` : summary,
				push: false
			})
			.then(() => {
				toasts.success('Commit created');
				reset();
			})
			.catch(() => {
				toasts.error('Failed to commit');
			})
			.finally(() => {
				isCommitting = false;
				modal.close();
			});
	};

	const onAutowrite = async () => {
		const diff = Object.values(diffs).join('\n').slice(0, 5000);

		const backupSummary = summary;
		const backupDescription = description;
		summary = '';
		description = '';

		isAutowriting = true;
		api.summarize
			.commit(user.access_token, {
				diff,
				uid: project.id
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
			})
			.catch(() => {
				summary = backupSummary;
				description = backupDescription;
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isAutowriting = false;
			});
	};

	let modal: Modal;
</script>

<Modal bind:this={modal} let:close>
	<form
		class="font-modal-stroke/50 flex w-[680px] flex-col rounded-lg border-[0.5px] border-modal-stroke bg-modal-background"
		on:submit|preventDefault={onCommit}
	>
		<header class="flex w-full items-center justify-between p-4">
			<h2 class="flex items-center gap-4">
				<IconGitBranch class="h-5 w-5" />
				<span class="line-height-5 text-white">{head}</span>
			</h2>

			<Button
				role="purple"
				height="small"
				disabled={isCommitting || !project.api?.sync}
				loading={isAutowriting}
				on:click={onAutowrite}
			>
				Autowrite
			</Button>
		</header>

		<div class="flex flex-col px-4">
			<!-- svelte-ignore a11y-autofocus -->
			<input
				autofocus
				name="commit-message"
				class="overflow-auto border-0 border-none bg-transparent p-1 text-xl text-zinc-100"
				type="text"
				placeholder="Commit message (required)"
				disabled={isAutowriting || isCommitting}
				bind:value={summary}
				required
			/>

			<textarea
				bind:value={description}
				name="commit-description"
				class="resize-none border-none bg-transparent p-1 text-lg text-zinc-400"
				placeholder="Commit description (optional)"
				disabled={isAutowriting || isCommitting}
				rows="6"
			/>
		</div>

		<footer class="flex items-center justify-between p-4">
			<div class="text-zinc-400">
				<Link
					on:click={modal?.close}
					disabled={isAutowriting || isCommitting}
					href="/projects/{project.id}/commit/"
				>
					{Object.keys(statuses).length} files changed
				</Link>
			</div>

			<div class="flex gap-2">
				<Button filled={false} outlined={true} on:click={close}>Cancel</Button>
				<Button type="submit" disabled={isAutowriting} role="primary" loading={isCommitting}>
					Commit
				</Button>
			</div>
		</footer>
	</form>
</Modal>

<style>
	footer {
		box-shadow: inset 0px 1px 0px rgba(0, 0, 0, 0.1);
	}
</style>
