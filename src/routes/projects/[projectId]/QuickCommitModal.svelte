<script lang="ts">
	import * as toasts from '$lib/toasts';
	import type { Project } from '$lib/api/ipc/projects';
	import type { User, getCloudApiClient } from '$lib/api/cloud/api';
	import { isUnstaged, type Status } from '$lib/api/git/statuses';
	import { commit, stage } from '$lib/api/git';
	import { Button, Link } from '$lib/components';
	import { IconGitBranch, IconSparkle } from '$lib/icons';
	import { Stats } from '$lib/components';
	import Overlay from '$lib/components/Overlay/Overlay.svelte';

	export function show() {
		modal.show();
	}

	export let project: Project;
	export let head: string;
	export let statuses: Record<string, Status>;
	export let diffs: Record<string, string>;
	export let user: User;
	export let cloud: ReturnType<typeof getCloudApiClient>;

	let summary = '';
	let description = '';
	let isAutowriting = false;
	let isCommitting = false;

	const stageAll = async () => {
		const paths = Object.entries(statuses)
			.filter((entry) => isUnstaged(entry[1]))
			.map(([path]) => path);
		if (paths.length === 0) return;
		await stage({
			projectId: project.id,
			paths
		});
	};

	$: [linesAdded, linesRemoved] = Object.values(diffs)
		.map((diff) => {
			let added = 0;
			let removed = 0;
			let isHeader = true;
			for (const line of diff.split('\n')) {
				if (isHeader) {
					if (line.startsWith('@@')) {
						isHeader = false;
					}
					continue;
				} else if (line.startsWith('+')) {
					added++;
				} else if (line.startsWith('-')) {
					removed++;
				}
			}
			return [added, removed];
		})
		.reduce((a, b) => [a[0] + b[0], a[1] + b[1]], [0, 0]);

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
		commit({
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
		cloud.summarize
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

	let modal: Overlay;
</script>

<Overlay bind:this={modal} let:close>
	<form
		class="modal modal-quick-commit font-modal-stroke/50 flex w-[680px] flex-col"
		on:submit|preventDefault={onCommit}
	>
		<header class="flex w-full items-center justify-between p-4">
			<h2 class="flex items-center gap-2">
				<IconGitBranch class="h-5 w-5 text-zinc-400" />
				<span class="line-height-5 text-zinc-300">{head}</span>
			</h2>
		</header>

		<div class="flex flex-col px-4">
			<!-- svelte-ignore a11y-autofocus -->
			<div class="flex w-full items-center justify-between gap-2">
				<input
					autocomplete="off"
					autocorrect="off"
					spellcheck="true"
					autofocus
					name="commit-message"
					contenteditable="true"
					class="quick-commit-input break-word outline-none-important w-full overflow-auto border-0 border-none bg-transparent p-1 text-xl text-zinc-100"
					type="text"
					placeholder="Commit message (required)"
					disabled={isAutowriting || isCommitting}
					bind:value={summary}
					required
				/>

				<Button
					color="purple"
					kind="outlined"
					disabled={isCommitting || !project.api?.sync}
					loading={isAutowriting}
					on:click={onAutowrite}
				>
					<IconSparkle class="h-5 w-5" />
				</Button>
			</div>

			<textarea
				autocomplete="off"
				autocorrect="off"
				spellcheck="true"
				bind:value={description}
				name="commit-description"
				class="quick-commit-input outline-none-important resize-none border-none bg-transparent p-1 text-lg text-zinc-400"
				placeholder="Commit description (optional)"
				disabled={isAutowriting || isCommitting}
				rows="6"
			/>
		</div>

		<footer class="flex items-center justify-between p-4">
			<div class="flex items-center gap-4">
				<Link
					on:click={modal?.close}
					disabled={isAutowriting || isCommitting}
					href="/projects/{project.id}/commit/"
				>
					{Object.keys(statuses).length} files changed
					<Stats added={linesAdded} removed={linesRemoved} />
				</Link>
			</div>

			<div class="flex gap-2">
				<Button kind="outlined" on:click={close}>Cancel</Button>
				<Button type="submit" disabled={isAutowriting} color="purple" loading={isCommitting}>
					Commit
				</Button>
			</div>
		</footer>
	</form>
</Overlay>

<style lang="postcss">
	.quick-commit-input {
		@apply outline-none focus:outline-none active:outline-none;
		outline: none;
	}
	.quick-commit-input:focus {
		outline: 0;
		outline-offset: 0;
		box-shadow: rgb(255, 255, 255) 0px 0px 0px 0px, rgba(37, 99, 235, 0) 0px 0px 0px 2px,
			rgba(0, 0, 0, 0) 0px 0px 0px 0px;
	}
	footer {
		box-shadow: inset 0px 1px 0px rgba(0, 0, 0, 0.1);
	}
</style>
