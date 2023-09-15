<script lang="ts">
	import * as toasts from '$lib/toasts';
	import { slide } from 'svelte/transition';
	import { invoke } from '@tauri-apps/api/tauri';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch, File } from '$lib/vbranches/types';
	import type { getCloudApiClient } from '$lib/api/cloud/api';
	import type { User } from '$lib/api/cloud';
	import { Button, Tooltip } from '$lib/components';
	import { IconAISparkles, IconLoading, IconTriangleDown, IconTriangleUp } from '$lib/icons';
	import {
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis
	} from '$lib/config/config';
	import { createEventDispatcher } from 'svelte';
	import type { Ownership } from '$lib/vbranches/ownership';

	export let projectId: string;
	export let branchController: BranchController;
	export let branch: Branch;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let user: User | null;
	export let ownership: Ownership;

	const dispatch = createEventDispatcher<{ close: null }>();

	let commitMessage: string;
	$: messageRows =
		Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10) + 2;

	function commit() {
		branchController.commitBranch({
			branch: branch.id,
			message: commitMessage,
			ownership: ownership.toString()
		});
	}

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	let annotateCommits = true;

	function checkCommitsAnnotated() {
		git_get_config({ key: 'gitbutler.utmostDiscretion' }).then((value) => {
			annotateCommits = value ? value === '0' : true;
		});
	}
	$: checkCommitsAnnotated();

	let isGeneratingCommigMessage = false;
	async function generateCommitMessage(files: File[]) {
		expanded = false;
		const diff = files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if (user === null) return;

		isGeneratingCommigMessage = true;
		cloud.summarize
			.commit(user.access_token, {
				diff,
				uid: projectId,
				brief: $commitGenerationExtraConcise,
				emoji: $commitGenerationUseEmojis
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
				commitMessage = description.length > 0 ? `${summary}\n\n${description}` : summary;
			})
			.catch(() => {
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommigMessage = false;
			});
	}
	let expanded = false;
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);
</script>

<div class="bg-color-3 flex w-full flex-col" transition:slide={{ duration: 150 }}>
	{#if annotateCommits}
		<div class="bg-blue-400 p-2 text-sm text-white">
			GitButler will be the committer of this commit.
			<a
				target="_blank"
				rel="noreferrer"
				class="font-bold"
				href="https://docs.gitbutler.com/features/virtual-branches/committer-mark">Learn more</a
			>
		</div>
	{/if}
	<div class="flex items-center">
		<textarea
			bind:value={commitMessage}
			on:dblclick|stopPropagation
			class="text-color-2 bg-color-5 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto border border-transparent p-2 font-mono outline-none focus:border-purple-600 focus:ring-0 dark:focus:border-purple-600"
			placeholder="Your commit message here"
			rows={messageRows}
			required
		/>
	</div>
	<div class="flex flex-grow justify-end gap-2 p-3 px-5">
		<div class="relative flex flex-grow">
			{#if cloudEnabled && user}
				<div
					class="bg-color-3 border-color-3 absolute flex h-fit w-fit flex-col items-center whitespace-nowrap rounded border
				"
				>
					<div class="flex h-6 flex-row items-center justify-center font-medium leading-5">
						<button
							disabled={isGeneratingCommigMessage}
							class="hover:bg-color-2 flex h-full items-center justify-center gap-1 rounded-l pl-1.5 pr-1"
							on:click={() => generateCommitMessage(branch.files)}
						>
							{#if isGeneratingCommigMessage}
								<IconLoading
									class="text-color-4 h-4 w-4 animate-spin fill-purple-600 dark:fill-purple-200"
								/>
							{:else}
								<IconAISparkles class="text-color-4 h-4 w-4"></IconAISparkles>
							{/if}
							<span>Generate message</span>
						</button>
						<button
							id="close-button"
							disabled={isGeneratingCommigMessage}
							class="hover:bg-color-2 flex h-full items-center justify-center rounded-r pl-1 pr-1.5"
							on:click={() => (expanded = !expanded)}
						>
							{#if expanded}
								<IconTriangleUp></IconTriangleUp>
							{:else}
								<IconTriangleDown></IconTriangleDown>
							{/if}
						</button>
					</div>
					{#if expanded}
						<div class="border-color-4 z-50 w-full border-t px-2 py-0.5">
							<label class="flex items-center gap-1">
								<input
									type="checkbox"
									bind:checked={$commitGenerationExtraConcise}
									on:change={(e) => {
										if (e.target instanceof HTMLInputElement) {
											commitGenerationExtraConcise.set(e.target.checked);
										}
									}}
								/>
								Extra concise
							</label>
							<label class="flex items-center gap-1">
								<input
									type="checkbox"
									bind:checked={$commitGenerationUseEmojis}
									on:change={(e) => {
										if (e.target instanceof HTMLInputElement) {
											commitGenerationUseEmojis.set(e.target.checked);
										}
									}}
								/>
								Use emojis
							</label>
						</div>
					{/if}
				</div>
			{:else}
				<Tooltip
					label="Summary generation requres that you are logged in and have cloud sync enabled for the project"
				>
					<Button
						disabled={true}
						tabindex={-1}
						kind="outlined"
						class="text-light-500"
						height="small"
						icon={IconAISparkles}
						loading={isGeneratingCommigMessage}
					>
						<span class="text-light-700">Generate message</span>
					</Button>
				</Tooltip>
			{/if}
		</div>
		<Button
			class="w-20"
			height="small"
			color="purple"
			id="commit-to-branch"
			on:click={() => {
				if (commitMessage) commit();
				commitMessage = '';
				dispatch('close');
			}}
		>
			Commit
		</Button>
	</div>
</div>
