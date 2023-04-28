<script lang="ts">
	import type { PageData } from './$types';
	import { Button } from '$lib/components';
	import { collapse } from '$lib/paths';
	import { derived, writable } from 'svelte/store';
	import { git, Status } from '$lib/api';
	import DiffViewer from '$lib/components/DiffViewer.svelte';
	import { error, success } from '$lib/toasts';
	import { fly } from 'svelte/transition';
	import { Dialog } from '$lib/components';
	import { log } from '$lib';
	import IconChevronUp from '$lib/components/icons/IconChevronUp.svelte';
	import IconChevronDown from '$lib/components/icons/IconChevronDown.svelte';

	export let data: PageData;
	const { statuses, diffs, user, api, projectId, project } = data;

	let fullContext = false;
	let context = 3;

	$: stagedFiles = Object.entries($statuses)
		.filter((status) => Status.isStaged(status[1]))
		.map(([path]) => path);
	$: unstagedFiles = Object.entries($statuses)
		.filter((status) => Status.isUnstaged(status[1]))
		.map(([path]) => path);

	let connectToCloudDialog: Dialog;
	let summary = '';
	let description = '';

	const selectedDiffPath = writable<string | undefined>(
		Object.keys($statuses)
			.sort((a, b) => a.localeCompare(b))
			.at(0)
	);
	statuses.subscribe((statuses) => {
		if ($selectedDiffPath && Object.keys(statuses).includes($selectedDiffPath)) return;
		$selectedDiffPath = Object.keys(statuses)
			.sort((a, b) => a.localeCompare(b))
			.at(0);
	});
	const selectedDiff = derived([diffs, selectedDiffPath], ([diffs, selectedDiffPath]) =>
		selectedDiffPath ? diffs[selectedDiffPath] : undefined
	);

	const nextFilePath = derived([statuses, selectedDiffPath], ([statuses, selectedDiffPath]) => {
		if (selectedDiffPath === undefined) return;
		const paths = Object.keys(statuses).sort((a, b) => a.localeCompare(b));
		const index = paths.indexOf(selectedDiffPath);
		if (index === paths.length - 1) return;
		return paths[index + 1];
	});

	const previousFilePath = derived([statuses, selectedDiffPath], ([statuses, selectedDiffPath]) => {
		if (selectedDiffPath === undefined) return;
		const paths = Object.keys(statuses).sort((a, b) => a.localeCompare(b));
		const index = paths.indexOf(selectedDiffPath);
		if (index === 0) return;
		return paths[index - 1];
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
		if (!isCloudEnabled) {
			connectToCloudDialog.show();
			return;
		}
		if ($user === undefined) return;

		const partialDiff = Object.fromEntries(
			Object.entries($diffs).filter(([key]) => $statuses[key] && Status.isStaged($statuses[key]))
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
					paths: unstagedFiles
				})
				.catch(() => {
					error('Failed to stage files');
				});
		} else {
			git
				.unstage({
					projectId,
					paths: stagedFiles
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
		} catch (e) {
			log.error(`Failed to update project sync status: ${e}`);
			error('Failed to update project sync status');
		}
	};

	$: isCommitEnabled = summary.length > 0 && stagedFiles.length > 0;
	$: isLoggedIn = $user !== undefined;
	$: isCloudEnabled = $project?.api?.sync;
	$: isSomeFilesSelected = stagedFiles.length > 0 && Object.keys($statuses).length > 0;
	$: isGenerateCommitEnabled = isLoggedIn && isSomeFilesSelected;
</script>

<Dialog bind:this={connectToCloudDialog}>
	<svelte:fragment slot="title">GitButler Cloud required</svelte:fragment>
	<div class="w-[640px]">
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
	</div>
	<svelte:fragment slot="controls" let:close>
		<Button filled={false} outlined={true} on:click={close}>Cancel</Button>
		<Button role="primary" on:click={() => enableProjectSync().finally(close)}>Connect</Button>
	</svelte:fragment>
</Dialog>
<div id="commit-page" class="flex h-full w-full">
	<div class="commit-panel-container flex h-full w-[424px] flex-col border-r  border-zinc-700">
		<form on:submit|preventDefault={onCommit} class="flex h-full  flex-col gap-4 px-4">
			<h1 class="pt-2 text-2xl font-bold">Commit</h1>
			<ul
				class="flex h-full w-full flex-col overflow-auto rounded border border-gb-700 bg-card-default"
			>
				<header class="flex w-full items-center rounded-tl rounded-tr bg-card-active p-2">
					<input
						type="checkbox"
						class="h-[15px] w-[15px] cursor-default disabled:opacity-50"
						on:click={onGroupCheckboxClick}
						checked={Object.keys($statuses).length > 0 &&
							stagedFiles.length === Object.keys($statuses).length}
						indeterminate={stagedFiles.length > 0 &&
							unstagedFiles.length > 0 &&
							Object.keys($statuses).length > 0}
						disabled={isCommitting || isGeneratingCommitMessage}
					/>
					<h1 class="m-auto flex">
						<span class="w-full text-center">{Object.keys($statuses).length} changed files</span>
					</h1>
				</header>

				<div class="changed-file-list-container h-100 overflow-y-auto">
					{#each Object.entries($statuses) as [path, status]}
						<li class="bg-card-default last:mb-1">
							<div
								class:bg-[#3356C2]={$selectedDiffPath === path}
								class:hover:bg-divider={$selectedDiffPath !== path}
								class="file-changed-item mx-1 mt-1 flex select-text  items-center gap-2 rounded bg-card-default px-1 py-1"
							>
								<input
									class="h-[15px] w-[15px] cursor-default disabled:opacity-50"
									disabled={isCommitting || isGeneratingCommitMessage}
									on:click|preventDefault={() => {
										Status.isStaged(status)
											? git.unstage({ projectId, paths: [path] }).catch(() => {
													error('Failed to unstage file');
											  })
											: git.stage({ projectId, paths: [path] }).catch(() => {
													error('Failed to stage file');
											  });
									}}
									name="path"
									type="checkbox"
									checked={Status.isStaged(status)}
									value={path}
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
				</div>
			</ul>

			<div class="bottom-controller-container flex flex-col gap-2 pb-4">
				<input
					autocomplete="off"
					autocorrect="off"
					spellcheck="true"
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

				<div class="commit-description-container relative h-36">
					{#if isGeneratingCommitMessage}
						<div
							in:fly={{ y: 8, duration: 500 }}
							out:fly={{ y: -8, duration: 500 }}
							class="generating-commit absolute top-0 right-0 bottom-0 left-0 rounded border-2 border-[#502E5C] "
						>
							<div
								class="generating-commit-message absolute  bottom-0 left-0 rounded-tr bg-[#782E94] bg-gradient-to-b from-[#623871] to-[#502E5C] py-1 px-2"
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
						class="
							h-full w-full resize-none rounded border border-zinc-600 bg-zinc-700 p-2 text-zinc-100 
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
					<Button
						role="purple"
						disabled={!isGenerateCommitEnabled}
						on:click={onGenerateCommitMessage}
						loading={isGeneratingCommitMessage}
					>
						✨ Autowrite
					</Button>

					<Button
						loading={isCommitting}
						disabled={!isCommitEnabled || isGeneratingCommitMessage}
						role="primary"
						type="submit"
					>
						Commit changes
					</Button>
				</div>
			</div>
		</form>
	</div>

	<div
		id="preview"
		class="relative m-2 flex w-2/3 flex-auto flex-col overflow-auto rounded border-[0.5px] border-gb-700 bg-card-default"
	>
		{#if $selectedDiffPath}
			{#if $selectedDiff}
				<header class="flex items-center gap-3 bg-card-active px-3 py-2">
					<span>{$selectedDiffPath}</span>

					<div class="flex items-center gap-1">
						<button
							on:click={selectPreviousFile}
							class="cursor-pointer rounded border border-zinc-500 bg-zinc-600 p-0.5"
							class:hover:bg-zinc-500={$hasPreviousFile}
							class:cursor-not-allowed={!$hasPreviousFile}
							class:text-zinc-500={!$hasPreviousFile}
						>
							<IconChevronUp class="h-4 w-4" />
						</button>
						<button
							on:click={selectNextFile}
							class="cursor-pointer rounded border border-zinc-500 bg-zinc-600 p-0.5"
							class:hover:bg-zinc-500={$hasNextFile}
							class:cursor-not-allowed={!$hasNextFile}
							class:text-zinc-500={!$hasNextFile}
						>
							<IconChevronDown class="h-4 w-4" />
						</button>
					</div>
				</header>

				<div id="code" class="flex-auto overflow-auto px-2">
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
					class="absolute bottom-0 flex w-full flex-col gap-4 overflow-hidden rounded-br rounded-bl border-t border-zinc-700 bg-[#2E2E32]/75 p-2 pt-4"
					style="
                border-width: 0.5px; 
                -webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
                backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
                background-color: rgba(24, 24, 27, 0.60);
                border: 0.5px solid rgba(63, 63, 70, 0.50);
            "
				>
					<div class="align-center flex flex-row-reverse gap-2">
						<button class="checkbox-button">
							<label
								for="full-context-checkbox"
								class="group block cursor-default rounded  transition-colors duration-200 ease-in-out hover:bg-zinc-700 "
							>
								<input
									type="checkbox"
									id="full-context-checkbox"
									bind:checked={fullContext}
									class="peer hidden"
								/>

								<svg
									fill="none"
									xmlns="http://www.w3.org/2000/svg"
									class="group h-8 w-8 rounded p-1.5 peer-checked:hidden"
								>
									<path
										d="M10.177 2.07944L13.073 5.21176C13.1081 5.24957 13.1319 5.2978 13.1416 5.35031C13.1513 5.40283 13.1464 5.45727 13.1274 5.50674C13.1084 5.55621 13.0763 5.59848 13.0351 5.62818C12.9939 5.65789 12.9455 5.67369 12.896 5.6736H10.75V7.0256C10.75 7.24074 10.671 7.44707 10.5303 7.5992C10.3897 7.75133 10.1989 7.8368 10 7.8368C9.80109 7.8368 9.61032 7.75133 9.46967 7.5992C9.32902 7.44707 9.25 7.24074 9.25 7.0256V5.6736H7.104C7.05449 5.67369 7.00607 5.65789 6.96487 5.62818C6.92368 5.59848 6.89157 5.55621 6.87261 5.50674C6.85365 5.45727 6.8487 5.40283 6.85838 5.35031C6.86806 5.2978 6.89195 5.24957 6.927 5.21176L9.823 2.07944C9.84622 2.05426 9.87381 2.03428 9.90418 2.02065C9.93456 2.00702 9.96712 2 10 2C10.0329 2 10.0654 2.00702 10.0958 2.02065C10.1262 2.03428 10.1538 2.05426 10.177 2.07944ZM9.25 12.9744C9.25 12.7593 9.32902 12.5529 9.46967 12.4008C9.61032 12.2487 9.80109 12.1632 10 12.1632C10.1989 12.1632 10.3897 12.2487 10.5303 12.4008C10.671 12.5529 10.75 12.7593 10.75 12.9744V14.3264H12.896C12.9455 14.3263 12.9939 14.3421 13.0351 14.3718C13.0763 14.4015 13.1084 14.4438 13.1274 14.4933C13.1464 14.5427 13.1513 14.5972 13.1416 14.6497C13.1319 14.7022 13.1081 14.7504 13.073 14.7882L10.177 17.9206C10.1538 17.9457 10.1262 17.9657 10.0958 17.9794C10.0654 17.993 10.0329 18 10 18C9.96712 18 9.93456 17.993 9.90418 17.9794C9.87381 17.9657 9.84622 17.9457 9.823 17.9206L6.927 14.7882C6.89195 14.7504 6.86806 14.7022 6.85838 14.6497C6.8487 14.5972 6.85365 14.5427 6.87261 14.4933C6.89157 14.4438 6.92368 14.4015 6.96487 14.3718C7.00607 14.3421 7.05449 14.3263 7.104 14.3264H9.25V12.9744ZM4.25 10.8112C4.44891 10.8112 4.63968 10.7257 4.78033 10.5736C4.92098 10.4215 5 10.2151 5 10C5 9.78486 4.92098 9.57852 4.78033 9.42639C4.63968 9.27426 4.44891 9.1888 4.25 9.1888H3.75C3.55109 9.1888 3.36032 9.27426 3.21967 9.42639C3.07902 9.57852 3 9.78486 3 10C3 10.2151 3.07902 10.4215 3.21967 10.5736C3.36032 10.7257 3.55109 10.8112 3.75 10.8112H4.25ZM8 10C8 10.2151 7.92098 10.4215 7.78033 10.5736C7.63968 10.7257 7.44891 10.8112 7.25 10.8112H6.75C6.55109 10.8112 6.36032 10.7257 6.21967 10.5736C6.07902 10.4215 6 10.2151 6 10C6 9.78486 6.07902 9.57852 6.21967 9.42639C6.36032 9.27426 6.55109 9.1888 6.75 9.1888H7.25C7.44891 9.1888 7.63968 9.27426 7.78033 9.42639C7.92098 9.57852 8 9.78486 8 10ZM10.25 10.8112C10.4489 10.8112 10.6397 10.7257 10.7803 10.5736C10.921 10.4215 11 10.2151 11 10C11 9.78486 10.921 9.57852 10.7803 9.42639C10.6397 9.27426 10.4489 9.1888 10.25 9.1888H9.75C9.55109 9.1888 9.36032 9.27426 9.21967 9.42639C9.07902 9.57852 9 9.78486 9 10C9 10.2151 9.07902 10.4215 9.21967 10.5736C9.36032 10.7257 9.55109 10.8112 9.75 10.8112H10.25ZM14 10C14 10.2151 13.921 10.4215 13.7803 10.5736C13.6397 10.7257 13.4489 10.8112 13.25 10.8112H12.75C12.5511 10.8112 12.3603 10.7257 12.2197 10.5736C12.079 10.4215 12 10.2151 12 10C12 9.78486 12.079 9.57852 12.2197 9.42639C12.3603 9.27426 12.5511 9.1888 12.75 9.1888H13.25C13.4489 9.1888 13.6397 9.27426 13.7803 9.42639C13.921 9.57852 14 9.78486 14 10ZM16.25 10.8112C16.4489 10.8112 16.6397 10.7257 16.7803 10.5736C16.921 10.4215 17 10.2151 17 10C17 9.78486 16.921 9.57852 16.7803 9.42639C16.6397 9.27426 16.4489 9.1888 16.25 9.1888H15.75C15.5511 9.1888 15.3603 9.27426 15.2197 9.42639C15.079 9.57852 15 9.78486 15 10C15 10.2151 15.079 10.4215 15.2197 10.5736C15.3603 10.7257 15.5511 10.8112 15.75 10.8112H16.25Z"
										fill="none"
										class="fill-zinc-100 p-4 group-hover:fill-zinc-200 "
									/>
								</svg>

								<svg
									fill="none"
									xmlns="http://www.w3.org/2000/svg"
									class="group hidden h-8 w-8 rounded p-1.5 peer-checked:block"
								>
									<path
										d="M10.177 2.07944L13.073 5.21176C13.1081 5.24957 13.1319 5.2978 13.1416 5.35031C13.1513 5.40283 13.1464 5.45727 13.1274 5.50674C13.1084 5.55621 13.0763 5.59848 13.0351 5.62818C12.9939 5.65789 12.9455 5.67369 12.896 5.6736H10.75V7.0256C10.75 7.24074 10.671 7.44707 10.5303 7.5992C10.3897 7.75133 10.1989 7.8368 10 7.8368C9.80109 7.8368 9.61032 7.75133 9.46967 7.5992C9.32902 7.44707 9.25 7.24074 9.25 7.0256V5.6736H7.104C7.05449 5.67369 7.00607 5.65789 6.96487 5.62818C6.92368 5.59848 6.89157 5.55621 6.87261 5.50674C6.85365 5.45727 6.8487 5.40283 6.85838 5.35031C6.86806 5.2978 6.89195 5.24957 6.927 5.21176L9.823 2.07944C9.84622 2.05426 9.87381 2.03428 9.90418 2.02065C9.93456 2.00702 9.96712 2 10 2C10.0329 2 10.0654 2.00702 10.0958 2.02065C10.1262 2.03428 10.1538 2.05426 10.177 2.07944ZM9.25 12.9744C9.25 12.7593 9.32902 12.5529 9.46967 12.4008C9.61032 12.2487 9.80109 12.1632 10 12.1632C10.1989 12.1632 10.3897 12.2487 10.5303 12.4008C10.671 12.5529 10.75 12.7593 10.75 12.9744V14.3264H12.896C12.9455 14.3263 12.9939 14.3421 13.0351 14.3718C13.0763 14.4015 13.1084 14.4438 13.1274 14.4933C13.1464 14.5427 13.1513 14.5972 13.1416 14.6497C13.1319 14.7022 13.1081 14.7504 13.073 14.7882L10.177 17.9206C10.1538 17.9457 10.1262 17.9657 10.0958 17.9794C10.0654 17.993 10.0329 18 10 18C9.96712 18 9.93456 17.993 9.90418 17.9794C9.87381 17.9657 9.84622 17.9457 9.823 17.9206L6.927 14.7882C6.89195 14.7504 6.86806 14.7022 6.85838 14.6497C6.8487 14.5972 6.85365 14.5427 6.87261 14.4933C6.89157 14.4438 6.92368 14.4015 6.96487 14.3718C7.00607 14.3421 7.05449 14.3263 7.104 14.3264H9.25V12.9744ZM4.25 10.8112C4.44891 10.8112 4.63968 10.7257 4.78033 10.5736C4.92098 10.4215 5 10.2151 5 10C5 9.78486 4.92098 9.57852 4.78033 9.42639C4.63968 9.27426 4.44891 9.1888 4.25 9.1888H3.75C3.55109 9.1888 3.36032 9.27426 3.21967 9.42639C3.07902 9.57852 3 9.78486 3 10C3 10.2151 3.07902 10.4215 3.21967 10.5736C3.36032 10.7257 3.55109 10.8112 3.75 10.8112H4.25ZM8 10C8 10.2151 7.92098 10.4215 7.78033 10.5736C7.63968 10.7257 7.44891 10.8112 7.25 10.8112H6.75C6.55109 10.8112 6.36032 10.7257 6.21967 10.5736C6.07902 10.4215 6 10.2151 6 10C6 9.78486 6.07902 9.57852 6.21967 9.42639C6.36032 9.27426 6.55109 9.1888 6.75 9.1888H7.25C7.44891 9.1888 7.63968 9.27426 7.78033 9.42639C7.92098 9.57852 8 9.78486 8 10ZM10.25 10.8112C10.4489 10.8112 10.6397 10.7257 10.7803 10.5736C10.921 10.4215 11 10.2151 11 10C11 9.78486 10.921 9.57852 10.7803 9.42639C10.6397 9.27426 10.4489 9.1888 10.25 9.1888H9.75C9.55109 9.1888 9.36032 9.27426 9.21967 9.42639C9.07902 9.57852 9 9.78486 9 10C9 10.2151 9.07902 10.4215 9.21967 10.5736C9.36032 10.7257 9.55109 10.8112 9.75 10.8112H10.25ZM14 10C14 10.2151 13.921 10.4215 13.7803 10.5736C13.6397 10.7257 13.4489 10.8112 13.25 10.8112H12.75C12.5511 10.8112 12.3603 10.7257 12.2197 10.5736C12.079 10.4215 12 10.2151 12 10C12 9.78486 12.079 9.57852 12.2197 9.42639C12.3603 9.27426 12.5511 9.1888 12.75 9.1888H13.25C13.4489 9.1888 13.6397 9.27426 13.7803 9.42639C13.921 9.57852 14 9.78486 14 10ZM16.25 10.8112C16.4489 10.8112 16.6397 10.7257 16.7803 10.5736C16.921 10.4215 17 10.2151 17 10C17 9.78486 16.921 9.57852 16.7803 9.42639C16.6397 9.27426 16.4489 9.1888 16.25 9.1888H15.75C15.5511 9.1888 15.3603 9.27426 15.2197 9.42639C15.079 9.57852 15 9.78486 15 10C15 10.2151 15.079 10.4215 15.2197 10.5736C15.3603 10.7257 15.5511 10.8112 15.75 10.8112H16.25Z"
										fill="none"
										class="fill-zinc-600 p-4 group-hover:fill-zinc-200 "
									/>
								</svg>
							</label>
						</button>
						{#if !fullContext}
							<div class="hunk-controller-container flex items-center gap-2">
								<p>Context:</p>
								<input
									type="number"
									bind:value={context}
									min="0"
									class="w-14 rounded py-1 pl-2 pr-1"
								/>
							</div>
						{/if}
					</div>
				</div>
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
