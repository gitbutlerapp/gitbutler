<script lang="ts">
	import type { PageData } from './$types';
	import { collapsable } from '$lib/paths';
	import { derived, writable } from 'svelte/store';
	import * as git from '$lib/git';
	import DiffViewer from '$lib/components/DiffViewer.svelte';
	import { error, success } from '$lib/toasts';
	import { IconRotateClockwise } from '$lib/components/icons';

	export let data: PageData;
	const { statuses, diffs, user, api, projectId } = data;

	let summary = '';
	let description = '';

	const selectedDiffPath = writable($statuses.at(0)?.path);
	const selectedDiff = derived(
		[diffs, selectedDiffPath],
		([diffs, selectedDiffPath]) => diffs[selectedDiffPath]
	);

	const reset = () => {
		summary = '';
		description = '';
	};

	let isCommitting = false;
	let isGeneratingCommitMessage = false;

	const onCommit = async (e: SubmitEvent) => {
		const form = e.target as HTMLFormElement;
		const formData = new FormData(form);
		const summary = formData.get('summary') as string;
		const description = formData.get('description') as string;

		isCommitting = true;
		git
			.commit({
				projectId,
				message: description.length > 0 ? `${summary}\n\n${description}` : summary,
				push: false
			})
			.then(() => {
				success('Commit created');
				reset();
			})
			.catch(() => {
				error('Failed to commit');
			})
			.finally(() => {
				isCommitting = false;
			});
	};

	const onGenerateCommitMessage = async () => {
		if ($user === undefined) return;

		const partialDiff = Object.fromEntries(
			Object.entries($diffs).filter(([key]) =>
				$statuses.some((status) => status.path === key && status.staged)
			)
		);
		const diff = Object.values(partialDiff).join('\n').slice(0, 5000);

		isGeneratingCommitMessage = true;
		api.summarize
			.commit($user.access_token, {
				diff,
				uid: projectId
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				summary = firstNewLine > -1 ? message.slice(0, firstNewLine) : message;
				description = firstNewLine > -1 ? message.slice(firstNewLine + 1) : '';
			})
			.catch(() => {
				error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommitMessage = false;
			});
	};

	const onGroupCheckboxClick = (e: Event) => {
		const target = e.target as HTMLInputElement;
		if (target.checked) {
			git
				.stage({
					projectId,
					paths: $statuses.filter(({ staged }) => !staged).map(({ path }) => path)
				})
				.catch(() => {
					error('Failed to stage files');
				});
		} else {
			git
				.unstage({
					projectId,
					paths: $statuses.filter(({ staged }) => staged).map(({ path }) => path)
				})
				.catch(() => {
					error('Failed to unstage files');
				});
		}
	};

	$: isCommitEnabled = summary.length > 0 && $statuses.filter(({ staged }) => staged).length > 0;
</script>

<div id="commit-page" class="flex h-full w-full gap-2 p-2">
	<div>
		<h1 class="px-2 py-1 text-xl font-bold">Commit</h1>

		<form on:submit|preventDefault={onCommit} class="flex w-1/3 min-w-[500px] flex-col gap-4">
			<ul class="flex w-full flex-col rounded border border-card-default bg-card-active">
				<header class="flex w-full items-center py-2 px-4">
					<input
						type="checkbox"
						class="cursor-default disabled:opacity-50"
						on:click={onGroupCheckboxClick}
						checked={$statuses.every(({ staged }) => staged)}
						indeterminate={$statuses.some(({ staged }) => staged) &&
							$statuses.some(({ staged }) => !staged) &&
							$statuses.length > 0}
						disabled={isCommitting || isGeneratingCommitMessage}
					/>
					<h1 class="m-auto flex">
						<span class="w-full text-center">{$statuses.length} changed files</span>
					</h1>
				</header>

				{#each $statuses as { path, staged }, i}
					<li
						class:bg-gb-700={$selectedDiffPath === path}
						class:hover:bg-divider={$selectedDiffPath !== path}
						class:border-b={i < $statuses.length - 1}
						class="file-changed-item flex cursor-text select-text items-center gap-2 border-gb-700 bg-card-default"
					>
						<input
							class="ml-4 cursor-default py-2 disabled:opacity-50"
							disabled={isCommitting || isGeneratingCommitMessage}
							on:click|preventDefault={() => {
								staged
									? git.unstage({ projectId, paths: [path] }).catch(() => {
											error('Failed to unstage file');
									  })
									: git.stage({ projectId, paths: [path] }).catch(() => {
											error('Failed to stage file');
									  });
							}}
							name="path"
							type="checkbox"
							checked={staged}
							value={path}
						/>
						<label class="flex w-full overflow-auto" for="path">
							<button
								disabled={isCommitting || isGeneratingCommitMessage}
								on:click|preventDefault={() => ($selectedDiffPath = path)}
								type="button"
								class="h-full w-full cursor-text select-auto py-2 pr-4 text-left font-mono disabled:opacity-50"
								use:collapsable={{ value: path, separator: '/' }}
							/>
						</label>
					</li>
				{/each}
			</ul>

			<input
				name="summary"
				class="rounded border border-card-default bg-card-active p-3 disabled:opacity-50"
				disabled={isGeneratingCommitMessage || isCommitting}
				type="text"
				placeholder="Summary (required)"
				bind:value={summary}
				required
			/>

			<textarea
				name="description"
				disabled={isGeneratingCommitMessage || isCommitting}
				class="rounded border border-card-default bg-card-active p-3 disabled:opacity-50"
				rows="10"
				placeholder="Description (optional)"
				bind:value={description}
			/>

			<div class="flex justify-between">
				{#if isCommitting}
					<div
						class="flex gap-1 rounded bg-[#2563EB] py-2 px-4 text-lg disabled:cursor-not-allowed disabled:opacity-50"
					>
						<IconRotateClockwise class="h-5 w-5 animate-spin" />
						<span>Comitting...</span>
					</div>
				{:else}
					<button
						disabled={!isCommitEnabled || isGeneratingCommitMessage}
						type="submit"
						class="rounded bg-[#2563EB] py-2 px-4 text-lg disabled:cursor-not-allowed disabled:opacity-50"
					>
						Commit changes
					</button>
				{/if}

				{#if isGeneratingCommitMessage}
					<div
						class="flex items-center gap-1 rounded bg-gradient-to-b from-[#623871] to-[#502E5C] py-2 px-4 text-lg disabled:cursor-not-allowed disabled:opacity-50"
					>
						<IconRotateClockwise class="h-5 w-5 animate-spin" />
						<span>Generating commit message...</span>
					</div>
				{:else}
					<button
						type="button"
						disabled={$user === undefined}
						class="rounded bg-gradient-to-b from-[#623871] to-[#502E5C] py-2 px-4 text-lg disabled:cursor-not-allowed disabled:opacity-50"
						on:click|preventDefault={onGenerateCommitMessage}
					>
						✨ Generate commit message
					</button>
				{/if}
			</div>
		</form>
	</div>

	<div id="preview" class="m-2 flex flex-auto cursor-text select-text overflow-auto">
		{#if $selectedDiff !== undefined}
			<DiffViewer diff={$selectedDiff} path={$selectedDiffPath} />
		{:else}
			<div class="flex h-full w-full flex-col items-center justify-center">
				<p class="text-lg">Unable to load diff</p>
			</div>
		{/if}
	</div>
</div>
