<script lang="ts">
	import * as toasts from '$lib/utils/toasts';
	import { slide } from 'svelte/transition';
	import { invoke } from '@tauri-apps/api/tauri';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch, File } from '$lib/vbranches/types';
	import type { getCloudApiClient } from '$lib/backend/cloud';
	import type { User } from '$lib/backend/cloud';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis
	} from '$lib/config/config';
	import { Ownership } from '$lib/vbranches/ownership';
	import Button from '$lib/components/Button.svelte';
	import TextArea from '$lib/components/TextArea.svelte';
	import DropDown from '$lib/components/DropDown.svelte';
	import InfoMessage from '$lib/components/InfoMessage.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { Writable } from 'svelte/store';
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher<{
		action: 'generate-branch-name';
	}>();

	export let projectId: string;
	export let branchController: BranchController;
	export let branch: Branch;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let user: User | undefined;
	export let selectedOwnership: Writable<Ownership>;

	const aiGenEnabled = projectAiGenEnabled(projectId);
	let commitMessage: string;

	$: messageRows =
		Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10) + 2;

	function commit() {
		selectedOwnership.set(Ownership.fromBranch(branch));
		branchController.commitBranch(branch.id, commitMessage, $selectedOwnership.toString());
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
		const diff = files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if (!user) return;

		// Branches get their names generated only if there are at least 4 lines of code
		// If the change is a 'one-liner', the branch name is either left as "virtual branch"
		// or the user has to manually trigger the name generation from the meatball menu
		// This saves people this extra click
		if (branch.name.toLowerCase().includes('virtual branch')) {
			dispatch('action', 'generate-branch-name');
		}
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
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);

	let contextMenu: ContextMenu;
</script>

<div class="commit-box" transition:slide={{ duration: 150 }}>
	{#if annotateCommits}
		<InfoMessage color="accent-dim">
			GitButler will be the committer of this commit. <a
				target="_blank"
				rel="noreferrer"
				href="https://docs.gitbutler.com/features/virtual-branches/committer-mark">Learn more</a
			>
		</InfoMessage>
	{/if}
	<TextArea bind:value={commitMessage} rows={messageRows} placeholder="Your commit message here" />
	<div class="actions">
		<Tooltip
			label={$aiGenEnabled && user
				? undefined
				: 'You must be logged in and have summary generation enabled to use this feature'}
		>
			<DropDown
				kind="outlined"
				disabled={!$aiGenEnabled || !user}
				loading={isGeneratingCommigMessage}
				on:click={() => generateCommitMessage(branch.files)}
			>
				Generate message
				<ContextMenu type="checklist" slot="popup" bind:this={contextMenu}>
					<ContextMenuItem
						checked={$commitGenerationExtraConcise}
						label="Extra concise"
						on:click={() => commitGenerationExtraConcise.update((value) => !value)}
					/>
					<ContextMenuItem
						checked={$commitGenerationUseEmojis}
						label="Use emojis 😎"
						on:click={() => commitGenerationUseEmojis.update((value) => !value)}
					/>
				</ContextMenu>
			</DropDown>
		</Tooltip>
		<Button
			color="primary"
			id="commit-to-branch"
			on:click={() => {
				if (commitMessage) commit();
				commitMessage = '';
			}}
		>
			Commit
		</Button>
	</div>
</div>

<style lang="postcss">
	.commit-box {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-theme-container-outline-light);
		background: var(--clr-theme-container-pale);
		padding: var(--space-16);
		gap: var(--space-8);
	}
	.actions {
		display: flex;
		justify-content: right;
		gap: var(--space-6);
	}
</style>
