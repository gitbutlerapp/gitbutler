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
	import { Dialog } from '$lib/components';
	import { log, toasts } from '$lib';

	export let data: PageData;
	const { statuses, diffs, user, api, projectId, project } = data;

	let connectToCloudDialog: Dialog;
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
		if (!isLoggedIn) {
			// TODO: Modal prompting the user to log in
			return;
		}
		if (!isCloudEnabled) {
			connectToCloudDialog.show();
			return;
		}
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

	const enableProjectSync = async () => {
		if ($project === undefined) return;
		if ($user === undefined) return;

		try {
			if (!$project.api) {
				const apiProject = await api.projects.create($user.access_token, {
					name: $project.title,
					uid: $project.id
				});
				await project.update({ api: { ...apiProject, sync: true } });
			} else {
				await project.update({ api: { ...$project.api, sync: true } });
			}
		} catch (error) {
			log.error(`Failed to update project sync status: ${error}`);
			toasts.error('Failed to update project sync status');
		}
	};

	$: isCommitEnabled = summary.length > 0 && $statuses.filter(({ staged }) => staged).length > 0;
	$: isLoggedIn = $user !== undefined;
	$: isCloudEnabled = $project?.api?.sync;
	$: isSomeFilesSelected = $statuses.some(({ staged }) => staged) && $statuses.length > 0;
	$: isGenerateCommitEnabled = isLoggedIn && isSomeFilesSelected;
</script>

<Dialog bind:this={connectToCloudDialog}>
	<svelte:fragment slot="title">GitButler Cloud required</svelte:fragment>
	<svelte:fragment>
		<p class="py-2">
			By connecting to GitButler Cloud you'll unlock improved, cloud only features, including
			AI-generated commit summaries, and the assurance of never losing your work with synced
			project.
		</p>
		<p class="flex flex-col py-2">
			<span class="font-semibold text-zinc-300">AI-genearate commits</span>
			<span>
				This not only saves you time and effort but also ensures consistency in tone and style,
				ultimately helping you to boost sales and improve customer satisfaction.
			</span>
		</p>
		<p class="flex flex-col py-2">
			<span class="font-semibold text-zinc-300">Secure and reliable backup</span>
			<span>
				GitButler backup guarantees that anything you’ve ever written in your projects are safe,
				secure and easily recoverable.
			</span>
		</p>
	</svelte:fragment>
	<svelte:fragment slot="controls" let:hide let:show>
		<Button filled on:click={hide}>Cancel</Button>
		<Button
			filled
			role="primary"
			on:click={() => {
				enableProjectSync();
				hide();
			}}>Connect</Button
		>
	</svelte:fragment>
</Dialog>
<div id="commit-page" class="flex h-full w-full">
	<div class="commit-panel-container border-r border-zinc-700 p-4">
		<h1 class="px-2 py-1 text-xl font-bold">Commit</h1>

		<form on:submit|preventDefault={onCommit} class="flex w-1/3 min-w-[500px] flex-col gap-4">
			<ul class="flex w-full flex-col rounded border border-gb-700 bg-card-default pb-1">
				<header class="flex w-full items-center bg-card-active p-2 rounded-tl rounded-tr">
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
					<li class="bg-card-default ">
						<div
							class:bg-[#3356C2]={$selectedDiffPath === path}
							class:hover:bg-divider={$selectedDiffPath !== path}
							class="file-changed-item mx-1 mt-1 flex select-text  items-center gap-2 rounded bg-card-default px-1 py-1"
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
									class="h-full w-full select-auto text-left font-mono text-sm disabled:opacity-50"
									use:collapsable={{ value: path, separator: '/' }}
								/>
							</label>
						</div>
					</li>
				{/each}
			</ul>

			<input
				name="summary"
				class="
					w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100 
					hover:border-zinc-500/80
					focus:border-[1px] focus:focus:border-blue-600 
					focus:ring-2 focus:ring-blue-600/30
				"
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
						class="generating-commit absolute top-0 right-0 bottom-1 left-0 rounded border-2 border-[#782E94]"
					>
						<div
							class="generating-commit-message absolute  bottom-0 left-0 rounded-tr bg-[#782E94] py-1 px-2"
						>
							<span>✨ Summarizing changes</span>
							<span class="dot-container">
								<div class="dot" />
								<div class="dot" />
								<div class="dot" />
							</span>
						</div>
					</div>
				{/if}
				<textarea
					name="description"
					disabled={isGeneratingCommitMessage || isCommitting}
					class="
						w-full rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100 
						hover:border-zinc-500/80
						focus:border-[1px] focus:focus:border-blue-600 
						focus:ring-2 focus:ring-blue-600/30
					"
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
