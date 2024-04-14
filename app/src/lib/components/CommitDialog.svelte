<script lang="ts">
	import Button from './Button.svelte';
	import { AIService } from '$lib/ai/service';
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
	import { showError } from '$lib/notifications/toasts';
	import { User } from '$lib/stores/user';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { setAutoHeight } from '$lib/utils/useAutoHeight';
	import { useResize } from '$lib/utils/useResize';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch, type LocalFile } from '$lib/vbranches/types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { fly, slide } from 'svelte/transition';
	import type { Writable } from 'svelte/store';

	const aiService = getContext(AIService);

	const dispatch = createEventDispatcher<{
		action: 'generate-branch-name';
	}>();

	export let projectId: string;
	export let expanded: Writable<boolean>;

	const branchController = getContext(BranchController);
	const selectedOwnership = getContextStore(Ownership);
	const branch = getContextStore(Branch);
	const user = getContextStore(User);

	const aiGenEnabled = projectAiGenEnabled(projectId);
	const runCommitHooks = projectRunCommitHooks(projectId);
	const commitMessage = persistedCommitMessage(projectId, $branch.id);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(projectId);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(projectId);

	let isCommitting = false;
	let aiLoading = false;

	let contextMenu: ContextMenu;

	let titleTextArea: HTMLTextAreaElement;
	let descriptionTextArea: HTMLTextAreaElement;

	$: ({ title, description } = splitMessage($commitMessage));
	$: if ($commitMessage) updateHeights();

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
				$branch.id,
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
		const hunks = files.flatMap((f) =>
			f.hunks.filter((h) => $selectedOwnership.contains(f.id, h.id))
		);
		// Branches get their names generated only if there are at least 4 lines of code
		// If the change is a 'one-liner', the branch name is either left as "virtual branch"
		// or the user has to manually trigger the name generation from the meatball menu
		// This saves people this extra click
		if ($branch.name.toLowerCase().includes('virtual branch')) {
			dispatch('action', 'generate-branch-name');
		}

		aiLoading = true;
		try {
			const generatedMessage = await aiService.summarizeCommit({
				hunks,
				useEmojiStyle: $commitGenerationUseEmojis,
				useBriefStyle: $commitGenerationExtraConcise,
				userToken: $user?.access_token
			});

			if (generatedMessage) {
				$commitMessage = generatedMessage;
			} else {
				throw new Error('Prompt generated no response');
			}
		} catch (e: any) {
			showError('Failed to generate commit message', e);
		} finally {
			aiLoading = false;
		}

		setTimeout(() => {
			updateHeights();
			descriptionTextArea.focus();
		}, 0);
	}

	let aiConfigurationValid = false;

	onMount(async () => {
		aiConfigurationValid = await aiService.validateConfiguration($user?.access_token);
	});
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

				{#if title.length > 0 || description}
					<textarea
						value={description}
						disabled={aiLoading}
						placeholder="Commit description (optional)"
						class="text-base-body-13 commit-box__textarea commit-box__textarea__description"
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
					use:tooltip={$aiGenEnabled && aiConfigurationValid
						? ''
						: 'You must be logged in or have provided your own API key and have summary generation enabled to use this feature'}
				>
					<DropDownButton
						style="ghost"
						kind="solid"
						icon="ai-small"
						disabled={!($aiGenEnabled && aiConfigurationValid)}
						loading={aiLoading}
						on:click={async () => generateCommitMessage($branch.files)}
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
				style="ghost"
				kind="solid"
				id="commit-to-branch"
				on:click={() => {
					$expanded = false;
				}}
			>
				Cancel
			</Button>
		{/if}
		<Button
			style="pop"
			kind="solid"
			grow
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
			{$expanded ? 'Commit' : 'Start commit'}
		</Button>
	</div>
</div>

<style lang="postcss">
	.commit-box {
		display: flex;
		flex-direction: column;
		padding: var(--size-14);
		background: var(--clr-bg-main);
		border-top: 1px solid var(--clr-border-main);
		transition: background-color var(--transition-medium);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}

	.commit-box__expander {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--size-12);
	}

	.commit-box__textarea-wrapper {
		display: flex;
		position: relative;
		padding: 0 0 var(--size-48);
		flex-direction: column;
		gap: var(--size-4);
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

		&::placeholder {
			color: oklch(from var(--clr-scale-ntrl-30) l c h / 0.4);
		}
	}

	.commit-box__textarea-tooltip {
		position: absolute;
		display: flex;
		bottom: var(--size-12);
		left: var(--size-12);
		padding: var(--size-2);
		border-radius: 100%;
		background: var(--clr-bg-alt);
		color: var(--clr-scale-ntrl-40);
	}

	.commit-box__textarea__title {
		padding: var(--size-12) var(--size-12) 0 var(--size-12);
	}

	.commit-box__textarea__description {
		padding: 0 var(--size-12) 0 var(--size-12);
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
		background-color: var(--clr-bg-alt);
	}
</style>
