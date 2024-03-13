<script lang="ts">
	import { AI_SERVICE_CONTEXT, type AIService } from '$lib/backend/aiService';
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
		persistedCommitMessage
	} from '$lib/config/config';
	import { getContextByClass } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { tooltip } from '$lib/utils/tooltip';
	import { setAutoHeight } from '$lib/utils/useAutoHeight';
	import { useResize } from '$lib/utils/useResize';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createEventDispatcher, getContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { fly, slide } from 'svelte/transition';
	import type { User } from '$lib/backend/cloud';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Branch, LocalFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	const aiService = getContext<AIService>(AI_SERVICE_CONTEXT);

	const dispatch = createEventDispatcher<{
		action: 'generate-branch-name';
	}>();

	export let projectId: string;
	export let branch: Branch;
	export let user: User | undefined;
	export let selectedOwnership: Writable<Ownership>;
	export let expanded: Writable<boolean>;

	const branchController = getContextByClass(BranchController);

	const aiGenEnabled = projectAiGenEnabled(projectId);
	const runCommitHooks = projectRunCommitHooks(projectId);
	const commitMessage = persistedCommitMessage(projectId, branch.id);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);

	let isCommitting = false;
	let aiLoading = false;

	let contextMenu: ContextMenu;

	let titleTextArea: HTMLTextAreaElement;
	let descriptionTextArea: HTMLTextAreaElement;

	$: [title, description] = splitMessage($commitMessage);
	$: if ($commitMessage) updateHeights();

	function splitMessage(message: string) {
		const parts = message.split(/\n+(.*)/s);
		return [parts[0] || '', parts[1] || ''];
	}

	function concatMessage(title: string, description: string) {
		return `${title}\n\n${description}`;
	}

	function focusTextareaOnMount(el: HTMLTextAreaElement) {
		el.focus();
	}

	function updateHeights() {
		setAutoHeight(titleTextArea);
		setAutoHeight(descriptionTextArea);
	}

	async function commit() {
		const message = concatMessage(title, description);
		isCommitting = true;
		try {
			await branchController.commitBranch(
				branch.id,
				message.trim(),
				$selectedOwnership.toString(),
				$runCommitHooks
			);
			$commitMessage = '';
		} finally {
			isCommitting = false;
		}
	}

	async function generateCommitMessage(files: LocalFile[]) {
		const diff = files
			.map((f) => f.hunks.filter((h) => $selectedOwnership.containsHunk(f.id, h.id)))
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		// Branches get their names generated only if there are at least 4 lines of code
		// If the change is a 'one-liner', the branch name is either left as "virtual branch"
		// or the user has to manually trigger the name generation from the meatball menu
		// This saves people this extra click
		if (branch.name.toLowerCase().includes('virtual branch')) {
			dispatch('action', 'generate-branch-name');
		}

		aiLoading = true;
		try {
			const generatedMessage = await aiService.commit(
				diff,
				$commitGenerationUseEmojis,
				$commitGenerationExtraConcise
			);

			if (generatedMessage) {
				$commitMessage = generatedMessage;
			} else {
				toasts.error('Failed to generate commit message');
			}
		} catch {
			toasts.error('Failed to generate commit message');
		} finally {
			aiLoading = false;
		}

		setTimeout(() => {
			updateHeights();
			descriptionTextArea.focus();
		}, 0);
	}
</script>

<div class="commit-box" class:commit-box__expanded={$expanded}>
	{#if $expanded}
		<div class="commit-box__expander" transition:slide={{ duration: 150, easing: quintOut }}>
			<div class="commit-box__textarea-wrapper text-input">
				<textarea
					value={title}
					placeholder="Commit summary"
					disabled={aiLoading}
					class="text-base-body-13 text-semibold commit-box__textarea commit-box__textarea__title"
					class:commit-box__textarea_bottom-padding={title.length == 0 && description.length == 0}
					spellcheck="false"
					rows="1"
					bind:this={titleTextArea}
					use:focusTextareaOnMount
					use:useResize={() => {
						setAutoHeight(titleTextArea);
					}}
					on:focus={(e) => setAutoHeight(e.currentTarget)}
					on:input={(e) => {
						$commitMessage = concatMessage(e.currentTarget.value, description);
					}}
					on:keydown={(e) => {
						if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') commit();
						if (e.key === 'Tab' || e.key === 'Enter') {
							e.preventDefault();
							descriptionTextArea.focus();
						}
					}}
				/>

				{#if title.length > 0}
					<textarea
						value={description}
						disabled={aiLoading}
						placeholder="Commit description (optional)"
						class="text-base-body-13 commit-box__textarea commit-box__textarea__description"
						class:commit-box__textarea_bottom-padding={description.length > 0 || title.length > 0}
						spellcheck="false"
						rows="1"
						bind:this={descriptionTextArea}
						use:useResize={() => setAutoHeight(descriptionTextArea)}
						on:focus={(e) => setAutoHeight(e.currentTarget)}
						on:input={(e) => {
							$commitMessage = concatMessage(title, e.currentTarget.value);
						}}
						on:keydown={(e) => {
							const value = e.currentTarget.value;
							if (e.key == 'Backspace' && value.length == 0) {
								e.preventDefault();
								titleTextArea.focus();
								setAutoHeight(e.currentTarget);
							} else if (e.key == 'a' && (e.metaKey || e.ctrlKey) && value.length == 0) {
								// select previous textarea on cmd+a if this textarea is empty
								e.preventDefault();
								titleTextArea.select();
							}
						}}
					/>
				{/if}

				{#if title.length > 50}
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
						loading={aiLoading}
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
			disabled={(isCommitting || !title || $selectedOwnership.isEmpty()) && $expanded}
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
		padding: var(--size-14);
		background: var(--clr-theme-container-light);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		transition: background-color var(--transition-medium);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}

	.commit-box__expander {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--size-12);
	}

	.commit-box__textarea-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
		gap: var(--size-4);
		padding: 0;
	}

	.commit-box__textarea {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--size-16);
		background: none;
		resize: none;
		&:focus {
			outline: none;
		}
	}

	.commit-box__textarea-tooltip {
		position: absolute;
		display: flex;
		bottom: var(--size-12);
		left: var(--size-12);
		padding: var(--size-2);
		border-radius: 100%;
		background: var(--clr-theme-container-pale);
		color: var(--clr-theme-scale-ntrl-40);
	}

	.commit-box__textarea__title {
		padding: var(--size-12) var(--size-12) 0 var(--size-12);
	}

	.commit-box__textarea__description {
		padding: 0 var(--size-12) var(--size-12) var(--size-12);
	}

	.commit-box__textarea_bottom-padding {
		padding-bottom: var(--size-48);
	}

	.commit-box__texarea-actions {
		position: absolute;
		display: flex;
		right: var(--size-12);
		bottom: var(--size-12);
	}

	.actions {
		display: flex;
		justify-content: right;
		gap: var(--size-6);
	}

	.commit-box__expanded {
		background-color: var(--clr-theme-container-pale);
	}
</style>
