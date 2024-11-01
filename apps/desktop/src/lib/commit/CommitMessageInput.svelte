<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis
	} from '$lib/config/config';
	import { showError } from '$lib/notifications/toasts';
	import { isFailure } from '$lib/result';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { KeyName } from '$lib/utils/hotkeys';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { VirtualBranch, LocalFile } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { isWhiteSpaceString } from '@gitbutler/ui/utils/string';
	import { createEventDispatcher, onMount, tick } from 'svelte';
	import { fly } from 'svelte/transition';

	export let isExpanded: boolean;
	export let commitMessage: string;
	export let valid: boolean = false;
	export let commit: (() => void) | undefined = undefined;
	export let cancel: () => void;

	const selectedOwnership = getContextStore(SelectedOwnership);
	const aiService = getContext(AIService);
	const branch = getContextStore(VirtualBranch);
	const project = getContext(Project);
	const promptService = getContext(PromptService);

	const dispatch = createEventDispatcher<{
		action: 'generate-branch-name';
	}>();

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(project.id);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(project.id);

	let aiLoading = false;
	let aiConfigurationValid = false;

	let titleTextArea: HTMLTextAreaElement | undefined;
	let descriptionTextArea: HTMLTextAreaElement | undefined;

	$: ({ title, description } = splitMessage(commitMessage));
	$: valid = !!title;

	function concatMessage(title: string, description: string) {
		return `${title}\n\n${description}`;
	}

	async function generateCommitMessage(files: LocalFile[]) {
		const hunks = files.flatMap((f) =>
			f.hunks.filter((h) => $selectedOwnership.isSelected(f.id, h.id))
		);
		// Branches get their names generated only if there are at least 4 lines of code
		// If the change is a 'one-liner', the branch name is either left as "virtual branch"
		// or the user has to manually trigger the name generation from the meatball menu
		// This saves people this extra click
		if ($branch.name.toLowerCase().includes('lane')) {
			dispatch('action', 'generate-branch-name');
		}

		aiLoading = true;

		const prompt = promptService.selectedCommitPrompt(project.id);

		let firstToken = true;

		const generatedMessageResult = await aiService.summarizeCommit({
			hunks,
			useEmojiStyle: $commitGenerationUseEmojis,
			useBriefStyle: $commitGenerationExtraConcise,
			commitTemplate: prompt,
			branchName: $branch.name,
			onToken: (t) => {
				if (firstToken) {
					commitMessage = '';
					firstToken = false;
				}
				commitMessage += t;
			}
		});

		if (isFailure(generatedMessageResult)) {
			showError('Failed to generate commit message', generatedMessageResult.failure);
			aiLoading = false;
			return;
		}

		const generatedMessage = generatedMessageResult.value;

		if (generatedMessage) {
			commitMessage = generatedMessage;
		} else {
			const errorMessage = 'Prompt generated no response';
			showError(errorMessage, undefined);
			aiLoading = false;
			return;
		}

		aiLoading = false;
	}

	onMount(async () => {
		aiConfigurationValid = await aiService.validateConfiguration();
	});

	function handleDescriptionKeyDown(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
		const value = e.currentTarget.value;

		if (e.key === KeyName.Escape) {
			e.preventDefault();
			cancel();
			return;
		}

		if (commit && (e.ctrlKey || e.metaKey) && e.key === KeyName.Enter) commit();

		if (e.key === KeyName.Delete && value.length === 0) {
			e.preventDefault();
			if (titleTextArea) {
				titleTextArea.focus();
				titleTextArea.selectionStart = titleTextArea.textLength;
			}
			return;
		}

		if (e.key === 'a' && (e.metaKey || e.ctrlKey) && value.length === 0) {
			// select previous textarea on cmd+a if this textarea is empty
			e.preventDefault();
			titleTextArea?.select();
			return;
		}
	}

	function handleSummaryKeyDown(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
		if (e.key === KeyName.Escape) {
			e.preventDefault();
			cancel();
			return;
		}

		if (commit && (e.ctrlKey || e.metaKey) && e.key === KeyName.Enter) commit();
		if (e.key === KeyName.Enter) {
			e.preventDefault();

			const caretStart = e.currentTarget.selectionStart;
			const caretEnd = e.currentTarget.selectionEnd;
			const value = e.currentTarget.value;

			// if the caret is not at the end of the text, move the rest of the text to the description
			// get rid of the selected text
			if (caretStart < value.length || caretEnd < value.length) {
				const toKeep = value.slice(0, caretStart);
				const toMove = value.slice(caretEnd);
				const newDescription = isWhiteSpaceString(description)
					? toMove
					: `${toMove}\n${description}`;
				commitMessage = concatMessage(toKeep, newDescription);
				tick().then(() => {
					if (descriptionTextArea) {
						descriptionTextArea.focus();
						descriptionTextArea.setSelectionRange(0, 0);
					}
				});
			}

			descriptionTextArea?.focus();
		}
	}

	export function focus() {
		titleTextArea?.focus();
	}
</script>

{#if isExpanded}
	<div class="commit-box__textarea-wrapper text-input">
		<Textarea
			value={title}
			unstyled
			placeholder="Commit summary"
			disabled={aiLoading}
			fontSize={13}
			padding={{ top: 12, right: 12, bottom: 0, left: 12 }}
			fontWeight="semibold"
			spellcheck="false"
			minRows={1}
			maxRows={10}
			bind:textBoxEl={titleTextArea}
			autofocus
			oninput={(e: InputEvent & { currentTarget: HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				commitMessage = concatMessage(target.value, description);
			}}
			onkeydown={handleSummaryKeyDown}
		/>

		{#if title.length > 0 || description}
			<Textarea
				value={description}
				unstyled
				placeholder="Commit description (optional)"
				disabled={aiLoading}
				fontSize={13}
				padding={{ top: 0, right: 12, bottom: 0, left: 12 }}
				spellcheck="false"
				minRows={1}
				maxRows={30}
				bind:textBoxEl={descriptionTextArea}
				oninput={(e: InputEvent & { currentTarget: HTMLTextAreaElement }) => {
					const target = e.currentTarget;
					commitMessage = concatMessage(title, target.value);
				}}
				onkeydown={handleDescriptionKeyDown}
			/>
		{/if}

		{#if title.length > 50}
			<Tooltip text={'50 characters or less is best.\nUse description for more details'}>
				<div transition:fly={{ y: 2, duration: 150 }} class="commit-box__textarea-tooltip">
					<Icon name="idea" />
				</div>
			</Tooltip>
		{/if}

		<Tooltip
			text={!aiConfigurationValid
				? 'You must be logged in or have provided your own API key'
				: !$aiGenEnabled
					? 'You must have summary generation enabled'
					: undefined}
		>
			<div class="commit-box__texarea-actions" class:commit-box-actions_expanded={isExpanded}>
				<DropDownButton
					style="ghost"
					outline
					icon="ai-small"
					disabled={!($aiGenEnabled && aiConfigurationValid)}
					loading={aiLoading}
					menuPosition="top"
					onclick={async () => await generateCommitMessage($branch.files)}
				>
					Generate message

					{#snippet contextMenuSlot()}
						<ContextMenuSection>
							<ContextMenuItem
								label="Extra concise"
								onclick={() => ($commitGenerationExtraConcise = !$commitGenerationExtraConcise)}
							>
								{#snippet control()}
									<Checkbox small bind:checked={$commitGenerationExtraConcise} />
								{/snippet}
							</ContextMenuItem>

							<ContextMenuItem
								label="Use emojis 😎"
								onclick={() => ($commitGenerationUseEmojis = !$commitGenerationUseEmojis)}
							>
								{#snippet control()}
									<Checkbox small bind:checked={$commitGenerationUseEmojis} />
								{/snippet}
							</ContextMenuItem>
						</ContextMenuSection>
					{/snippet}
				</DropDownButton>
			</div>
		</Tooltip>
	</div>
{/if}

<style lang="postcss">
	.commit-box__textarea-wrapper {
		display: flex;
		position: relative;
		padding: 0 0 48px;
		flex-direction: column;
		gap: 4px;
		animation: expand-box 0.17s ease-out forwards;
		/* props to animate on mount */
		max-height: 40px;
		opacity: 0;
	}

	@keyframes expand-box {
		from {
			opacity: 0;
			max-height: 40px;
			padding: 0 0 0;
		}
		to {
			opacity: 1;
			max-height: 600px;
			padding: 0 0 48px;
		}
	}

	.commit-box__textarea-tooltip {
		position: absolute;
		bottom: 12px;
		left: 12px;

		display: flex;
		align-items: center;
		justify-content: center;
		width: var(--size-tag);
		height: var(--size-tag);

		padding: 2px;
		border-radius: var(--radius-m);
		background: var(--clr-theme-ntrl-soft);
		color: var(--clr-scale-ntrl-50);
	}

	.commit-box__texarea-actions {
		position: absolute;
		right: 12px;
		bottom: 12px;
		/* props to animate on mount */
		display: none;
		opacity: 0;
		transform: translateY(10px);
	}

	/* MODIFIERS */

	.commit-box-actions_expanded {
		display: flex;
		animation: expand-actions 0.17s ease-out forwards;
		animation-delay: 0.1s;
	}

	@keyframes expand-actions {
		from {
			opacity: 0;
			transform: translateY(10px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
