<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
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
	import { useAutoHeight } from '$lib/utils/useAutoHeight';
	import { invoke } from '@tauri-apps/api/tauri';
	import { createEventDispatcher } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { get } from 'svelte/store';
	import { slide } from 'svelte/transition';
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

	const aiGenEnabled = projectAiGenEnabled(projectId);
	const runCommitHooks = projectRunCommitHooks(projectId);
	const currentCommitMessage = projectCurrentCommitMessage(projectId, branch.id);
	export const expanded = persisted<boolean>(false, 'commitBoxExpanded_' + branch.id);

	let commitMessage: string = get(currentCommitMessage) || '';
	let isCommitting = false;
	let textareaElement: HTMLTextAreaElement;
	$: if (textareaElement && commitMessage && expanded) {
		textareaElement.style.height = 'auto';
		textareaElement.style.height = `${textareaElement.scrollHeight + 2}px`;
	}

	const focusTextareaOnMount = (el: HTMLTextAreaElement) => {
		if (el) {
			el.focus();
		}
	};

	function commit() {
		isCommitting = true;
		branchController
			.commitBranch(branch.id, commitMessage, $selectedOwnership.toString(), $runCommitHooks)
			.then(() => {
				commitMessage = '';
				currentCommitMessage.set('');
			})
			.finally(() => (isCommitting = false));
	}

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	let isGeneratingCommigMessage = false;
	async function generateCommitMessage(files: LocalFile[]) {
		const diff = files
			.map((f) => f.hunks.filter((h) => $selectedOwnership.containsHunk(f.id, h.id)))
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
				currentCommitMessage.set(commitMessage);

				setTimeout(() => {
					textareaElement.focus();
				}, 0);
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

<div class="commit-box" class:commit-box__expanded={$expanded}>
	{#if $expanded}
		<div class="commit-box__expander" transition:slide={{ duration: 150, easing: quintOut }}>
			<div class="commit-box__textarea-wrapper">
				<textarea
					bind:this={textareaElement}
					bind:value={commitMessage}
					use:focusTextareaOnMount
					on:input={useAutoHeight}
					on:focus={useAutoHeight}
					on:change={() => currentCommitMessage.set(commitMessage)}
					spellcheck={false}
					class="commit-box__textarea text-base-body-13"
					rows="1"
					disabled={isGeneratingCommigMessage}
					placeholder="Your commit message here"
				/>

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
						loading={isGeneratingCommigMessage}
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
			disabled={(isCommitting || !commitMessage || $selectedOwnership.isEmpty()) && $expanded}
			id="commit-to-branch"
			on:click={() => {
				if ($expanded) {
					if (commitMessage) {
						commit();
					}
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
		padding: var(--space-16);
		background: var(--clr-theme-container-light);
		border-top: 1px solid var(--clr-theme-container-outline-light);
		transition: background-color var(--transition-medium);
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}
	.commit-box__expander {
		display: flex;
		flex-direction: column;
		margin-bottom: var(--space-12);
		/* overflow: hidden; */
	}
	.commit-box__textarea-wrapper {
		position: relative;
		display: flex;
		flex-direction: column;
	}
	.commit-box__textarea {
		overflow: hidden;
		/* box-sizing: border-box; */
		display: flex;
		flex-direction: column;
		color: var(--clr-theme-scale-ntrl-0);
		background: var(--clr-theme-container-light);
		padding: var(--space-12) var(--space-12) var(--space-48) var(--space-12);
		align-items: flex-end;
		gap: var(--space-16);

		border-radius: var(--radius-s) var(--radius-s) 0 0;
		border: 1px solid var(--clr-theme-container-outline-light);

		transition: border-color var(--transition-fast);
		resize: none;

		&:hover {
			border-color: color-mix(
				in srgb,
				var(--clr-theme-container-outline-light),
				var(--darken-dark)
			);
		}
		&:focus-within {
			border-color: color-mix(
				in srgb,
				var(--clr-theme-container-outline-light),
				var(--darken-extradark)
			);
			outline: none;
		}
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
