<script lang="ts">
	import * as toasts from '$lib/toasts';
	import { slide } from 'svelte/transition';
	import { invoke } from '@tauri-apps/api/tauri';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch, File } from '$lib/vbranches/types';
	import type { getCloudApiClient } from '$lib/api/cloud/api';
	import type { User } from '$lib/api/cloud';
	import { Button, Tooltip } from '$lib/components';
	import { IconAISparkles } from '$lib/icons';
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
	$: messageRows = Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10);

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
	function trimNonLetters(input: string): string {
		const regex = /^[^a-zA-Z]+|[^a-zA-Z]+$/g;
		const trimmedString = input.replace(regex, '');
		return trimmedString;
	}
	async function generateCommitMessage(files: File[]) {
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
				uid: projectId
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
				commitMessage = trimNonLetters(
					description.length > 0 ? `${summary}\n\n${description}` : summary
				);
			})
			.catch(() => {
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommigMessage = false;
			});
	}
</script>

<div
	class="flex w-full flex-col border-t border-light-400 bg-light-200 dark:border-dark-400 dark:bg-dark-800"
	transition:slide={{ duration: 150 }}
>
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
			class="flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto border border-white bg-white p-2 font-mono text-dark-700 outline-none focus:border-purple-600 focus:ring-0 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400"
			placeholder="Your commit message here"
			rows={messageRows}
			required
		/>
	</div>
	<div class="flex flex-grow justify-end gap-2 p-3 px-5">
		<div>
			{#if cloudEnabled && user}
				<Button
					disabled={isGeneratingCommigMessage}
					tabindex={-1}
					kind="outlined"
					class="text-light-500"
					height="small"
					id="generate-ai-message"
					icon={IconAISparkles}
					loading={isGeneratingCommigMessage}
					on:click={() => generateCommitMessage(branch.files)}
				>
					<span class="text-light-700">Generate message</span>
				</Button>
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
