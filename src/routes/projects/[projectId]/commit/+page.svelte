<script lang="ts">
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { collapsable } from '$lib/paths';
	import { derived, writable } from 'svelte/store';
	import * as git from '$lib/git';
	import DiffViewer from '$lib/components/DiffViewer.svelte';
	import { error, success } from '$lib/toasts';
	import { fly } from 'svelte/transition';
	import { IconRotateClockwise } from '$lib/components/icons';

	export let data: PageData;
	const { statuses, diffs, user, api, projectId } = data;

	let summary = '';
	let description = '';

	const selectedDiffPath = writable<string | undefined>($statuses.at(0)?.path);
	statuses.subscribe((statuses) => {
		$selectedDiffPath = statuses.at(0)?.path;
	});
	const selectedDiff = derived([diffs, selectedDiffPath], ([diffs, selectedDiffPath]) =>
		selectedDiffPath ? diffs[selectedDiffPath] : undefined
	);

	const reset = () => {
		summary = '';
		description = '';
		selectedDiffPath.set(undefined);
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

		const backupSummary = summary;
		const backupDescription = description;
		summary = '';
		description = '';

		isGeneratingCommitMessage = true;
		api.summarize
			.commit($user.access_token, {
				diff,
				uid: projectId
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
			})
			.catch(() => {
				summary = backupSummary;
				description = backupDescription;
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
	$: isLoggedIn = $user !== undefined;
	$: isSomeFilesSelected = $statuses.some(({ staged }) => staged) && $statuses.length > 0;
	$: isGenerateCommitEnabled = isLoggedIn && isSomeFilesSelected;
</script>

<div id="commit-page" class="flex h-full w-full">
	<div class="commit-panel-container border-r border-zinc-700 p-4">
		<h1 class="px-2 py-1 text-xl font-bold">Commit</h1>

		<form on:submit|preventDefault={onCommit} class="flex w-1/3 min-w-[500px] flex-col gap-4">
			<ul class="flex w-full flex-col rounded border border-gb-700 bg-card-active">
				<header class="flex w-full items-center p-2">
					<input
						type="checkbox"
						class="h-[15px] w-[15px] cursor-default disabled:opacity-50"
						on:click={onGroupCheckboxClick}
						checked={$statuses.every(({ staged }) => staged) && $statuses.length > 0}
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
						class="file-changed-item flex cursor-text select-text items-center gap-2 border-gb-700 bg-card-default px-2 py-2"
					>
						<input
							class="h-[15px] w-[15px] cursor-default disabled:opacity-50"
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
						<label class="flex h-5 w-full overflow-auto" for="path">
							<button
								disabled={isCommitting || isGeneratingCommitMessage}
								on:click|preventDefault={() => ($selectedDiffPath = path)}
								type="button"
								class="h-full w-full cursor-text select-auto text-left font-mono disabled:opacity-50"
								use:collapsable={{ value: path, separator: '/' }}
							/>
						</label>
					</li>
				{/each}
			</ul>

			<input
				name="summary"
				class="w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100 ring-blue-600/30 focus:border-blue-600 "
				disabled={isGeneratingCommitMessage || isCommitting}
				type="text"
				placeholder="Summary (required)"
				bind:value={summary}
				required
			/>

			<div class="commit-description-container relative">
				{#if isGeneratingCommitMessage}
					<div
						in:fly={{ y: 8, duration: 500 }}
						out:fly={{ y: -8, duration: 500 }}
						class="generating-commit absolute top-2 left-2 rounded border-[0.5px] border-[#52305F] bg-[#583366] px-3 py-1 shadow"
					>
						<span>✨ Summarizing changes</span>
						<span class="dot-container">
							<div class="dot" />
							<div class="dot" />
							<div class="dot" />
						</span>
					</div>
				{/if}
				<textarea
					name="description"
					disabled={isGeneratingCommitMessage || isCommitting}
					class="w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100  focus:border-blue-600"
					rows="10"
					placeholder="Description (optional)"
					bind:value={description}
				/>
			</div>

			<div class="flex justify-between">
				{#if isGeneratingCommitMessage}
					<div
						class="flex items-center gap-1 rounded bg-gradient-to-b from-[#623871] to-[#502E5C] py-2 px-4 disabled:cursor-not-allowed disabled:opacity-50"
						style="
							border-top: 1px solid rgba(255, 255, 255, 0.2);
							border-bottom: 1px solid rgba(0, 0, 0, 0.3);
							border-left: 1px solid rgba(255, 255, 255, 0);
							border-right: 1px solid rgba(255, 255, 255, 0);
							text-shadow: 0px 2px #00000021;
						"
					>
						<IconRotateClockwise class="h-5 w-5 animate-spin" />
						Generating commit message
					</div>
				{:else}
					<button
						type="button"
						disabled={!isGenerateCommitEnabled}
						class="rounded bg-gradient-to-b from-[#623871] to-[#502E5C] py-2 px-4 disabled:cursor-not-allowed disabled:opacity-50"
						style="
							border-top: 1px solid rgba(255, 255, 255, 0.2);
							border-bottom: 1px solid rgba(0, 0, 0, 0.3);
							border-left: 1px solid rgba(255, 255, 255, 0);
							border-right: 1px solid rgba(255, 255, 255, 0);
							text-shadow: 0px 2px #00000021;
						"
						on:click|preventDefault={onGenerateCommitMessage}
					>
						✨ Generate commit message
					</button>
				{/if}

				<Button
					loading={isCommitting}
					disabled={!isCommitEnabled || isGeneratingCommitMessage}
					role="primary"
					type="submit"
				>
					Commit changes
				</Button>
			</div>
		</form>
	</div>

	<div
		id="preview"
		class="m-4 flex flex-auto cursor-text select-text overflow-auto rounded border border-gb-700 bg-card-default p-4"
	>
		{#if $selectedDiffPath !== undefined}
			{#if $selectedDiff !== undefined}
				<DiffViewer diff={$selectedDiff} path={$selectedDiffPath} />
			{:else}
				<div class="flex h-full w-full flex-col items-center justify-center">
					<p class="text-lg">Unable to load diff</p>
				</div>
			{/if}
		{:else}
			<div class="flex h-full w-full flex-col items-center justify-center">
				<p class="text-lg">Select a file to preview changes</p>
			</div>
		{/if}
	</div>
</div>

<style>
	/**
	* ==============================================
	* Dot Typing
	* ==============================================
	*/
	.dot-container {
		padding-left: 4px;
		padding-bottom: 3px;
	}
	.dot {
		@apply bg-zinc-200;
		display: inline-block;
		width: 3px;
		height: 3px;
		border-radius: 50%;
		position: relative;
		bottom: 3px;
	}

	.dot-container .dot:nth-last-child(1) {
		animation: jumpingAnimation 1.2s 0.6s linear infinite;
	}
	.dot-container .dot:nth-last-child(2) {
		animation: jumpingAnimation 1.2s 0.3s linear infinite;
	}
	.dot-container .dot:nth-last-child(3) {
		animation: jumpingAnimation 1.2s 0s linear infinite;
	}

	@keyframes jumpingAnimation {
		0% {
			transform: translate(0, 0);
		}
		16% {
			transform: translate(0, -5px);
		}
		33% {
			transform: translate(0, 0);
		}
		100% {
			transform: translate(0, 0);
		}
	}
</style>
