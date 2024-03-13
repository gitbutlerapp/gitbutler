<script lang="ts">
	import { ButlerAIProvider } from '$lib/backend/aiProviders';
	import { Summarizer } from '$lib/backend/summarizer';
	import Button from '$lib/components/Button.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis,
		projectRunCommitHooks,
		projectCurrentCommitMessage
	} from '$lib/config/config';
	import { persisted } from '$lib/persisted/persisted';
	import * as toasts from '$lib/utils/toasts';
	import { tooltip } from '$lib/utils/tooltip';
	import { setAutoHeight, useAutoHeight } from '$lib/utils/useAutoHeight';
	import { useResize } from '$lib/utils/useResize';
	import { invoke } from '@tauri-apps/api/tauri';
	import { createEventDispatcher } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { fly, slide } from 'svelte/transition';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Branch, LocalFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	const dispatch = createEventDispatcher<{
		action: 'generate-branch-name';
	}>();

	export let projectId: string;
	export let branchController: BranchController;
	export let branch: Branch;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let user: User | undefined;
	export let selectedOwnership: Writable<Ownership>;
	export const expanded = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);

	const aiGenEnabled = projectAiGenEnabled(projectId);
	const runCommitHooks = projectRunCommitHooks(projectId);
	const currentCommitMessage = projectCurrentCommitMessage(projectId, branch.id);

	function getCommitMessageTitleAndDescription(commitMessage: string) {
		// Split the commit message into title and description
		// get the first line as title and the rest as description
		const [summary, description] = commitMessage.trim().split(/\n+(.*)/s);
		console.log('summary and description', summary, description);
		return {
			summary: summary || '',
			description: description || ''
		};
	}

	function concatCommitMessage(message: { summary: string; description: string }) {
		return `${message.summary}\n${message.description}`;
	}

	let commitMessageSet = getCommitMessageTitleAndDescription($currentCommitMessage);
	let isCommitting = false;

	let summaryTextareaElement: HTMLTextAreaElement;
	let descriptionTextareaElement: HTMLTextAreaElement;

	// need to detect if the textareas wrapper is focused
	let isTextareaFocused = false;
	let isSecondTextareaFocused = false;

	function focusTextareaOnMount(el: HTMLTextAreaElement) {
		if (el) el.focus();
	}

	async function commit() {
		if (!commitMessageSet.summary) return;
		isCommitting = true;
		branchController
			.commitBranch(
				branch.id,
				concatCommitMessage(commitMessageSet),
				$selectedOwnership.toString(),
				$runCommitHooks
			)
			.then(() => {
				commitMessageSet.summary = '';
				commitMessageSet.description = '';
				currentCommitMessage.set('');
			})
			.finally(() => (isCommitting = false));
	}

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	let summarizer: Summarizer | undefined;

	$: if (user) {
		const aiProvider = new ButlerAIProvider(cloud, user);

		summarizer = new Summarizer(aiProvider);
	}

	let isGeneratingCommitMessage = false;
	async function generateCommitMessage(files: LocalFile[]) {
		const diff = files
			.map((f) => f.hunks.filter((h) => $selectedOwnership.containsHunk(f.id, h.id)))
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if (!user) return;
		if (!summarizer) return;

		// Branches get their names generated only if there are at least 4 lines of code
		// If the change is a 'one-liner', the branch name is either left as "virtual branch"
		// or the user has to manually trigger the name generation from the meatball menu
		// This saves people this extra click
		if (branch.name.toLowerCase().includes('virtual branch')) {
			dispatch('action', 'generate-branch-name');
		}

		isGeneratingCommitMessage = true;
		summarizer
			.commit(diff, $commitGenerationUseEmojis, $commitGenerationExtraConcise)
			.then((message) => {
				commitMessageSet = getCommitMessageTitleAndDescription(message);
				currentCommitMessage.set(message);

				setTimeout(() => {
					summaryTextareaElement.focus();
					setAutoHeight(summaryTextareaElement);
					setAutoHeight(descriptionTextareaElement);
				}, 0);
			})
			.catch(() => {
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommitMessage = false;
			});
	}
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);

	let contextMenu: ContextMenu;

	// concat commit message set into a single commit message
	$: if (commitMessageSet.summary.length > 0) {
		currentCommitMessage.set(concatCommitMessage(commitMessageSet));
	}
	// move description to title if title is empty
	$: if (commitMessageSet.summary.length == 0 && commitMessageSet.description.length > 0) {
		const messageSet = getCommitMessageTitleAndDescription(commitMessageSet.description);
		commitMessageSet.summary = messageSet.summary;
		commitMessageSet.description = messageSet.description;
	}
	// set auto height on textareas on each message change
	$: if (commitMessageSet) {
		setAutoHeight(summaryTextareaElement);
		setAutoHeight(descriptionTextareaElement);
	}
</script>

<div class="commit-box" class:commit-box__expanded={$expanded}>
	{#if $expanded}
		<div class="commit-box__expander" transition:slide={{ duration: 150, easing: quintOut }}>
			<div
				class="commit-box__textarea-wrapper text-input"
				class:text-input__focused={$expanded && (isTextareaFocused || isSecondTextareaFocused)}
			>
				<textarea
					bind:this={summaryTextareaElement}
					bind:value={commitMessageSet.summary}
					use:focusTextareaOnMount
					use:useResize={() => {
						setAutoHeight(summaryTextareaElement);
						setAutoHeight(descriptionTextareaElement);
					}}
					on:input={useAutoHeight}
					on:focus={(e) => {
						useAutoHeight(e);
						isTextareaFocused = true;
					}}
					on:blur={() => {
						isTextareaFocused = false;
					}}
					on:keydown={(e) => {
						if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
							commit();
						}

						if (e.key === 'Tab' || e.key === 'Enter') {
							e.preventDefault();
							descriptionTextareaElement.focus();
						}
					}}
					spellcheck={false}
					class="text-base-body-13 text-semibold commit-box__textarea commit-box__textarea__title"
					class:commit-box__textarea_bottom-padding={commitMessageSet.description.length == 0 &&
						commitMessageSet.summary.length == 0}
					rows="1"
					disabled={isGeneratingCommitMessage}
					placeholder="Commit summary"
				/>

				{#if commitMessageSet.summary.length > 0}
					<textarea
						bind:this={descriptionTextareaElement}
						bind:value={commitMessageSet.description}
						on:focus={(e) => {
							useAutoHeight(e);
							isSecondTextareaFocused = true;
						}}
						on:blur={() => {
							isSecondTextareaFocused = false;
						}}
						use:setAutoHeight
						spellcheck={false}
						class="text-base-body-13 commit-box__textarea commit-box__textarea__description"
						class:commit-box__textarea_bottom-padding={commitMessageSet.summary.length > 0}
						rows="1"
						disabled={isGeneratingCommitMessage}
						placeholder="Commit description (optional)"
						on:keydown={(e) => {
							if (commitMessageSet.description.length == 0 && e.key === 'Backspace') {
								e.preventDefault();
								summaryTextareaElement.focus();
							}

							// select previous textarea on cmd+a if this textarea is empty
							if (
								e.key === 'a' &&
								(e.metaKey || e.ctrlKey) &&
								commitMessageSet.description.length == 0
							) {
								e.preventDefault();
								summaryTextareaElement.focus();
								summaryTextareaElement.setSelectionRange(0, summaryTextareaElement.value.length);
							}
						}}
					/>
				{/if}

				{#if commitMessageSet.summary.length > 50}
					<div
						transition:fly={{ y: 2, duration: 150 }}
						class="commit-box__textarea-tooltip"
						use:tooltip={{
							text: '50 characters or less is best. Extra info can be added in the description.',
							delay: 200
						}}
					>
						<Icon name="blitz" />
					</div>
				{/if}

				<div
					class="commit-box__texarea-actions"
					use:tooltip={$aiGenEnabled && user
						? ''
						: 'You must be logged in and have summary generation enabled to use this feature'}
				>
					<DropDownButton
						kind="outlined"
						icon="ai-small"
						color="neutral"
						disabled={!$aiGenEnabled || !user}
						loading={isGeneratingCommitMessage}
						on:click={() => generateCommitMessage(branch.files)}
					>
						Generate message
						<ContextMenu type="checklist" slot="context-menu" bind:this={contextMenu}>
							<ContextMenuSection>
								<ContextMenuItem
									label="Extra concise"
									on:click={() => ($commitGenerationExtraConcise = !$commitGenerationExtraConcise)}
								>
									<Checkbox small slot="control" bind:checked={$commitGenerationExtraConcise} />
								</ContextMenuItem>

								<ContextMenuItem
									label="Use emojis 😎"
									on:click={() => ($commitGenerationUseEmojis = !$commitGenerationUseEmojis)}
								>
									<Checkbox small slot="control" bind:checked={$commitGenerationUseEmojis} />
								</ContextMenuItem>
							</ContextMenuSection>
						</ContextMenu>
					</DropDownButton>
				</div>
			</div>
		</div>
	{/if}
	<div class="actions">
		{#if $expanded && !isCommitting}
			<Button
				color="neutral"
				kind="outlined"
				id="commit-to-branch"
				on:click={() => {
					$expanded = false;
				}}
			>
				Cancel
			</Button>
		{/if}
		<Button
			grow
			color="primary"
			kind="filled"
			loading={isCommitting}
			disabled={(isCommitting || !commitMessageSet.summary || $selectedOwnership.isEmpty()) &&
				$expanded}
			id="commit-to-branch"
			on:click={() => {
				if ($expanded) {
					commit();
				} else {
					$expanded = true;
				}
			}}
		>
			{$expanded ? 'Commit' : 'Commit changes'}
		</Button>
	</div>
</div>

<style lang="postcss">
	.commit-box {
		display: flex;
		flex-direction: column;
		padding: var(--space-14);
		background: var(--clr-theme-container-light);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		transition: background-color var(--transition-medium);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
	.commit-box__expander {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--space-12);
	}
	.commit-box__textarea-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		/* padding: var(--space-12) var(--space-12) var(--space-48) var(--space-12); */
		padding: 0;
	}
	.commit-box__textarea {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--space-16);
		background: none;

		resize: none;

		&:focus {
			outline: none;
		}
	}

	.commit-box__textarea-tooltip {
		position: absolute;
		display: flex;
		bottom: var(--space-12);
		left: var(--space-12);
		padding: var(--space-2);
		border-radius: 100%;
		background: var(--clr-theme-container-pale);
		color: var(--clr-theme-scale-ntrl-40);
	}

	.commit-box__textarea__title {
		padding: var(--space-12) var(--space-12) 0 var(--space-12);
	}

	.commit-box__textarea__description {
		padding: 0 var(--space-12) var(--space-12) var(--space-12);
	}

	.commit-box__textarea_bottom-padding {
		padding-bottom: var(--space-48);
	}

	.commit-box__texarea-actions {
		position: absolute;
		display: flex;
		right: var(--space-12);
		bottom: var(--space-12);
	}

	.actions {
		display: flex;
		justify-content: right;
		gap: var(--space-6);
	}

	/* modifiers */
	.commit-box__expanded {
		background-color: var(--clr-theme-container-pale);
	}
</style>
