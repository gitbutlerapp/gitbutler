<script lang="ts">
	import type { PageData } from './$types';
	import { Button, Checkbox, DiffContext } from '$lib/components';
	import { collapse } from '$lib/paths';
	import { derived, writable } from '@square/svelte-store';
	import { isStaged, isUnstaged } from '$lib/api/git/statuses';
	import { userStore } from '$lib/stores/user';
	import { commit, stage, unstage } from '$lib/api/git';
	import DiffViewer from './DiffViewer.svelte';
	import { page } from '$app/stores';
	import { error, success } from '$lib/toasts';
	import { fly } from 'svelte/transition';
	import { Modal } from '$lib/components';
	import * as hotkeys from '$lib/hotkeys';
	import { IconChevronDown, IconChevronUp } from '$lib/icons';
	import { onMount } from 'svelte';
	import { unsubscribe } from '$lib/utils';

	export let data: PageData;
	let { statuses, diffs, cloud, project } = data;

	const user = userStore;

	let fullContext = false;
	let context = 3;

	const stagedFiles = derived(statuses, (statuses) =>
		Object.entries(statuses ?? {})
			.filter((status) => isStaged(status[1]))
			.map(([path]) => path)
	);
	const unstagedFiles = derived(statuses, (statuses) =>
		Object.entries(statuses ?? {})
			.filter((status) => isUnstaged(status[1]))
			.map(([path]) => path)
	);
	const allFiles = derived(statuses, (statuses) =>
		Object.keys(statuses ?? {}).sort((a, b) => a.localeCompare(b))
	);

	let connectToCloudModal: Modal;
	let summary = '';
	let description = '';

	const selectedDiffPath = writable<string | undefined>(
		Object.keys($statuses ?? {})
			.sort((a, b) => a.localeCompare(b))
			.at(0)
	);
	statuses.subscribe((statuses) => {
		if ($selectedDiffPath && Object.keys(statuses ?? {}).includes($selectedDiffPath)) return;
		$selectedDiffPath = Object.keys(statuses ?? {})
			.sort((a, b) => a.localeCompare(b))
			.at(0);
	});
	const selectedDiff = derived([diffs, selectedDiffPath], ([diffs, selectedDiffPath]) =>
		diffs && selectedDiffPath ? diffs[selectedDiffPath] : undefined
	);

	const nextFilePath = derived([allFiles, selectedDiffPath], ([files, selectedDiffPath]) => {
		if (selectedDiffPath === undefined) return;
		const index = files.indexOf(selectedDiffPath);
		if (index === files.length - 1) return;
		return files[index + 1];
	});

	const previousFilePath = derived([allFiles, selectedDiffPath], ([files, selectedDiffPath]) => {
		if (selectedDiffPath === undefined) return;
		const index = files.indexOf(selectedDiffPath);
		if (index === 0) return;
		return files[index - 1];
	});

	const selectNextFile = () => {
		if ($nextFilePath) $selectedDiffPath = $nextFilePath;
	};
	const selectPreviousFile = () => {
		if ($previousFilePath) $selectedDiffPath = $previousFilePath;
	};
	const hasNextFile = derived(nextFilePath, (nextFilePath) => nextFilePath !== undefined);
	const hasPreviousFile = derived(
		previousFilePath,
		(previousFilePath) => previousFilePath !== undefined
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
		commit({
			projectId: $page.params.projectId,
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
		if (!isCloudEnabled) {
			connectToCloudModal.show();
			return;
		}
		if ($user === null) return;

		const partialDiff = Object.fromEntries(
			Object.entries($diffs ?? {}).filter(([key]) => $statuses[key] && isStaged($statuses[key]))
		);
		const diff = Object.values(partialDiff).join('\n').slice(0, 5000);

		const backupSummary = summary;
		const backupDescription = description;
		summary = '';
		description = '';

		isGeneratingCommitMessage = true;
		cloud.summarize
			.commit($user.access_token, {
				diff,
				uid: $page.params.projectId
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
			stage({
				projectId: $page.params.projectId,
				paths: $unstagedFiles
			}).catch(() => {
				error('Failed to stage files');
			});
		} else {
			unstage({
				projectId: $page.params.projectId,
				paths: $stagedFiles
			}).catch(() => {
				error('Failed to unstage files');
			});
		}
	};

	const enableProjectSync = async () => {
		if ($project === undefined) return;
		if ($user === null) return;

		try {
			if (!$project.api) {
				const apiProject = await cloud.projects.create($user.access_token, {
					name: $project.title,
					uid: $project.id
				});
				await project.update({ api: { ...apiProject, sync: true } });
			} else {
				await project.update({ api: { ...$project.api, sync: true } });
			}
		} catch (e) {
			console.error(`Failed to update project sync status: ${e}`);
			error('Failed to update project sync status');
		}
	};

	$: isCommitEnabled = summary.length > 0 && $stagedFiles.length > 0;
	$: isLoggedIn = $user !== null;
	$: isCloudEnabled = $project?.api?.sync;
	$: isSomeFilesSelected = $stagedFiles.length > 0 && $allFiles.length > 0;
	$: isGenerateCommitEnabled = isLoggedIn && isSomeFilesSelected;

	// a situation where a file is created, then added to git index, and then deleted
	// is not handled by our UI very good. to simplify things, we just stage the file
	// which effectively removes it from the UI and keeps consistency between our ui
	// an git
	statuses.subscribe((statuses) =>
		Object.entries(statuses ?? {}).forEach(([file, status]) => {
			const isStagedAdded = isStaged(status) && status.staged === 'added';
			const isUnstagedDeleted = isUnstaged(status) && status.unstaged === 'deleted';
			if (isStagedAdded && isUnstagedDeleted)
				stage({ projectId: $page.params.projectId, paths: [file] });
		})
	);

	onMount(() =>
		unsubscribe(
			hotkeys.on('ArrowUp', () => selectPreviousFile()),
			hotkeys.on('Control+n', () => selectPreviousFile()),
			hotkeys.on('k', () => selectPreviousFile()),
			hotkeys.on('ArrowDown', () => selectNextFile()),
			hotkeys.on('Control+p', () => selectNextFile()),
			hotkeys.on('j', () => selectNextFile())
		)
	);
</script>

<Modal bind:this={connectToCloudModal} title="GitButler Cloud required">
	<div class="flex flex-col gap-2">
		<p>
			By connecting to GitButler Cloud you'll unlock improved, cloud only features, including
			AI-generated commit summaries, and the assurance of never losing your work with synced
			project.
		</p>

		<span class="font-semibold text-zinc-300">AI-genearate commits</span>
		<p class="flex flex-col">
			This not only saves you time and effort but also ensures consistency in tone and style,
			ultimately helping you to boost sales and improve customer satisfaction.
		</p>

		<span class="font-semibold text-zinc-300">Secure and reliable backup</span>
		<p>
			GitButler backup guarantees that anything you’ve ever written in your projects are safe,
			secure and easily recoverable.
		</p>
	</div>

	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button color="purple" on:click={() => enableProjectSync().finally(close)}>Connect</Button>
	</svelte:fragment>
</Modal>

<div id="commit-page" class="flex h-full w-full">
	<div class="commit-panel-container side-panel flex flex-col">
		<form on:submit|preventDefault={onCommit} class="flex h-full flex-col gap-4 px-4">
			<h1 class="pt-2 text-2xl font-bold">Commit</h1>
			<ul class="card flex h-full w-full flex-col overflow-auto">
				<header class="flex w-full items-center rounded-tl rounded-tr bg-card-active p-2">
					{#await Promise.all([stagedFiles.load(), unstagedFiles.load(), allFiles.load()]) then}
						<Checkbox
							checked={$allFiles.length > 0 && $stagedFiles.length === $allFiles.length}
							indeterminate={$stagedFiles.length > 0 &&
								$unstagedFiles.length > 0 &&
								$allFiles.length > 0}
							disabled={isCommitting || isGeneratingCommitMessage}
							on:click={onGroupCheckboxClick}
						/>

						<h1 class="m-auto flex">
							<span class="w-full text-center">{$allFiles.length} changed files</span>
						</h1>
					{/await}
				</header>

				<div class="changed-file-list-container h-100 overflow-y-auto">
					{#await Promise.all([statuses.load(), selectedDiffPath.load()]) then}
						{#each Object.entries($statuses).sort( (a, b) => a[0].localeCompare(b[0]) ) as [path, status]}
							<li class="bg-card-default last:mb-1">
								<div
									class:bg-[#3356C2]={$selectedDiffPath === path}
									class:hover:bg-divider={$selectedDiffPath !== path}
									class="file-changed-item mx-1 mt-1 flex select-text items-center gap-2 rounded bg-card-default px-1 py-1"
								>
									<Checkbox
										checked={isStaged(status)}
										name="path"
										disabled={isCommitting || isGeneratingCommitMessage}
										value={path}
										on:click={() => {
											isStaged(status)
												? unstage({ projectId: $page.params.projectId, paths: [path] }).catch(
														() => {
															error('Failed to unstage file');
														}
												  )
												: stage({ projectId: $page.params.projectId, paths: [path] }).catch(() => {
														error('Failed to stage file');
												  });
										}}
									/>
									<label class="flex h-5 w-full overflow-auto" for="path">
										<button
											disabled={isCommitting || isGeneratingCommitMessage}
											on:click|preventDefault={() => ($selectedDiffPath = path)}
											type="button"
											class="h-full w-full select-auto text-left font-mono text-base disabled:opacity-50"
										>
											{collapse(path)}
										</button>
									</label>
								</div>
							</li>
						{/each}
					{/await}
				</div>
			</ul>

			<div class="bottom-controller-container flex flex-col gap-2 pb-4">
				<input
					autocomplete="off"
					autocorrect="off"
					spellcheck="true"
					name="summary"
					class="w-full"
					disabled={isGeneratingCommitMessage || isCommitting}
					type="text"
					placeholder="Summary (required)"
					bind:value={summary}
					required
				/>

				<div class="commit-description-container relative h-36">
					{#if isGeneratingCommitMessage}
						<div
							in:fly={{ y: 8, duration: 500 }}
							out:fly={{ y: -8, duration: 500 }}
							class="generating-commit absolute bottom-0 left-0 right-0 top-0 rounded border-2 border-[#502E5C]"
						>
							<div
								class="generating-commit-message absolute bottom-0 left-0 rounded-tr bg-[#782E94] bg-gradient-to-b from-[#623871] to-[#502E5C] px-2 py-1"
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
						autocomplete="off"
						autocorrect="off"
						spellcheck="true"
						name="description"
						disabled={isGeneratingCommitMessage || isCommitting}
						class="h-full w-full resize-none"
						rows="10"
						placeholder="Description (optional)"
						bind:value={description}
					/>
				</div>

				<div class="flex justify-between">
					<Button
						color="purple"
						disabled={!isGenerateCommitEnabled}
						on:click={onGenerateCommitMessage}
						loading={isGeneratingCommitMessage}
					>
						✨ Autowrite
					</Button>

					<Button
						loading={isCommitting}
						disabled={!isCommitEnabled || isGeneratingCommitMessage}
						color="purple"
						type="submit"
					>
						Commit changes
					</Button>
				</div>
			</div>
		</form>
	</div>

	<div class="main-content-container">
		<div id="preview" class="card relative m-2 flex h-full flex-col overflow-auto">
			{#await Promise.all([selectedDiffPath.load(), selectedDiff.load()])}
				<div class="flex h-full w-full flex-col items-center justify-center">
					<p class="text-lg">Loading...</p>
				</div>
			{:then}
				{#if !$selectedDiffPath}
					<div class="flex h-full w-full flex-col items-center justify-center">
						<p class="text-lg">Select a file to preview changes</p>
					</div>
				{:else if !$selectedDiff}
					<div class="flex h-full w-full flex-col items-center justify-center">
						<p class="text-lg">Unable to load diff</p>
					</div>
				{:else}
					<header class="flex items-center gap-3 bg-card-active py-2 pl-2 pr-3">
						<div class="flex items-center gap-1">
							<button
								on:click={selectPreviousFile}
								class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
								class:hover:bg-zinc-500={$hasPreviousFile}
								class:cursor-not-allowed={!$hasPreviousFile}
								class:text-zinc-500={!$hasPreviousFile}
							>
								<IconChevronUp class="h-4 w-4" />
							</button>
							<button
								on:click={selectNextFile}
								class="rounded border border-zinc-500 bg-zinc-600 p-0.5"
								class:hover:bg-zinc-500={$hasNextFile}
								class:cursor-not-allowed={!$hasNextFile}
								class:text-zinc-500={!$hasNextFile}
							>
								<IconChevronDown class="h-4 w-4" />
							</button>
						</div>

						<span>{$selectedDiffPath}</span>
					</header>

					<div id="code" class="flex-auto overflow-auto bg-[#1E2021]">
						<div class="pb-[65px]">
							<DiffViewer
								diff={$selectedDiff ?? ''}
								path={$selectedDiffPath}
								paddingLines={fullContext ? 10000 : context}
							/>
						</div>
					</div>

					<div
						id="controls"
						class="absolute bottom-0 flex w-full flex-col gap-4 overflow-hidden rounded-bl rounded-br border-t border-zinc-700 bg-[#2E2E32]/75 p-2 pt-4"
						style="
							border-width: 0.5px; 
							-webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
							backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
							background-color: rgba(24, 24, 27, 0.60);
							border: 0.5px solid rgba(63, 63, 70, 0.50);
						"
					>
						<DiffContext bind:lines={context} bind:fullContext />
					</div>
				{/if}
			{/await}
		</div>
	</div>
</div>

<style lang="postcss">
	.changed-file-list-container {
		max-height: calc(100vh - 200px);
	}

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
