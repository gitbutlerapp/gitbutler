<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { AIService, type DiffInput } from '$lib/ai/service';
	import { BranchStack } from '$lib/branches/branch';
	import { SelectedOwnership } from '$lib/branches/ownership';
	import { DetailedCommit, Commit } from '$lib/commits/commit';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis,
		projectRunCommitHooks
	} from '$lib/config/config';
	import { FileService } from '$lib/files/fileService';
	import { HooksService } from '$lib/hooks/hooksService';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import * as toasts from '@gitbutler/ui/toasts';
	import { KeyName } from '@gitbutler/ui/utils/hotkeys';
	import { isWhiteSpaceString } from '@gitbutler/ui/utils/string';
	import { onMount, tick } from 'svelte';

	interface Props {
		existingCommit?: DetailedCommit | Commit;
		isExpanded: boolean;
		commitMessage: string;
		valid?: boolean;
		commit?: (() => void) | undefined;
		cancel: () => void;
	}

	let {
		isExpanded,
		existingCommit,
		commitMessage = $bindable(),
		valid = $bindable(false),
		commit = undefined,
		cancel
	}: Props = $props();

	const selectedOwnership = getContextStore(SelectedOwnership);
	const aiService = getContext(AIService);
	const stack = getContextStore(BranchStack);
	const project = getContext(Project);
	const promptService = getContext(PromptService);
	const hooksService = getContext(HooksService);
	const fileService = getContext(FileService);

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(project.id);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(project.id);
	const runHooks = projectRunCommitHooks(project.id);

	let aiLoading = $state(false);
	let hookRunning = $state(false);
	let aiConfigurationValid = $state(false);

	let titleTextArea: HTMLTextAreaElement | undefined = $state();
	let descriptionTextArea: HTMLTextAreaElement | undefined = $state();
	let isTitleFocused = $state(true);
	let isDescriptionFocused = $state(false);

	const { title, description } = $derived(splitMessage(commitMessage, true));

	$effect(() => {
		valid = !!title;
	});

	function concatMessage(title: string, description: string) {
		return `${title}\n\n${description}`;
	}

	async function getDiffInput(): Promise<DiffInput[]> {
		if (!existingCommit) {
			return $stack.files.flatMap((f) =>
				f.hunks
					.filter((h) => $selectedOwnership.isSelected(f.id, h.id))
					.map((h) => ({
						filePath: f.path,
						diff: h.diff
					}))
			);
		}

		const files = await fileService.listCommitFiles(project.id, existingCommit.id);
		return files.flatMap((file) =>
			file.hunks.map((hunk) => ({
				filePath: file.path,
				diff: hunk.diff
			}))
		);
	}

	async function generateCommitMessage() {
		aiLoading = true;
		try {
			const diffInput = await getDiffInput();

			const prompt = promptService.selectedCommitPrompt(project.id);

			let firstToken = true;

			const output = await aiService.summarizeCommit({
				diffInput,
				useEmojiStyle: $commitGenerationUseEmojis,
				useBriefStyle: $commitGenerationExtraConcise,
				commitTemplate: prompt,
				branchName: $stack.series[0]?.name,
				onToken: (t) => {
					if (firstToken) {
						commitMessage = '';
						firstToken = false;
					}
					commitMessage += t;
				}
			});

			if (output) {
				commitMessage = output;
			}
		} finally {
			aiLoading = false;
		}
	}

	async function runMessageHook() {
		hookRunning = true;
		try {
			const result = await hooksService.message(project.id, commitMessage);
			if (result.status === 'message') {
				commitMessage = result.message;
				toasts.success('Message hook modified your message');
			} else if (result.status === 'failure') {
				showError('Message hook failed', result.error);
			}
		} catch (err: unknown) {
			showError('Message hook failed', err);
		} finally {
			hookRunning = false;
		}
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

		if (commit && (e.ctrlKey || e.metaKey) && e.key === KeyName.Enter) {
			commit();
			return;
		}

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

{#snippet charCounter(tooltip: string, count: number)}
	<Tooltip text={tooltip}>
		<span class="text-11 text-semibold text-body commit-box__textarea-char-counter-label">
			{count}
		</span>
	</Tooltip>
{/snippet}

{#if isExpanded}
	<div class="commit-box__textarea-wrapper text-input">
		<Textarea
			value={title}
			unstyled
			placeholder="Commit summary"
			disabled={aiLoading}
			fontSize={13}
			padding={{ top: 12, right: 28, bottom: 0, left: 12 }}
			fontWeight="semibold"
			spellcheck={false}
			flex="1"
			minRows={1}
			maxRows={10}
			bind:textBoxEl={titleTextArea}
			autofocus
			onfocus={() => {
				isTitleFocused = true;
				isDescriptionFocused = false;
			}}
			onblur={() => {
				isTitleFocused = false;
				if ($runHooks) {
					runMessageHook();
				}
			}}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
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
				spellcheck={false}
				minRows={1}
				maxRows={30}
				bind:textBoxEl={descriptionTextArea}
				onfocus={() => {
					isDescriptionFocused = true;
					isTitleFocused = false;
				}}
				onblur={() => {
					isDescriptionFocused = false;
					if ($runHooks) {
						runMessageHook();
					}
				}}
				oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
					const target = e.currentTarget;
					commitMessage = concatMessage(title, target.value);
				}}
				onkeydown={handleDescriptionKeyDown}
			/>
		{/if}

		{#if title.length > 0 && isTitleFocused}
			{@render charCounter('Summary chars', title.length)}
		{/if}

		{#if description.length > 0 && isDescriptionFocused}
			{@render charCounter('Description chars', description.length)}
		{/if}

		<Tooltip
			text={!aiConfigurationValid
				? 'Log in or provide your own API key'
				: !$aiGenEnabled
					? 'Enable summary generation'
					: undefined}
		>
			<div class="commit-box__texarea-actions" class:commit-box-actions_expanded={isExpanded}>
				{#if hookRunning}
					<Icon name="spinner" opacity={0.4} />
				{/if}
				<DropDownButton
					kind="outline"
					icon="ai-small"
					disabled={!($aiGenEnabled && aiConfigurationValid)}
					loading={aiLoading}
					menuPosition="top"
					onclick={async () => await generateCommitMessage()}
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
		flex-direction: column;
		/* props to animate on mount */
		max-height: 40px;
		padding: 0 0 48px;
		gap: 4px;
		animation: expand-box 0.17s ease-out forwards;
		opacity: 0;
	}

	@keyframes expand-box {
		from {
			max-height: 40px;
			padding: 0 0 0;
			opacity: 0;
		}
		to {
			max-height: 600px;
			padding: 0 0 48px;
			opacity: 1;
		}
	}

	.commit-box__textarea-char-counter-label {
		position: absolute;
		top: 2px;
		right: 4px;
		padding: 4px;
		color: var(--clr-text-3);
	}

	.commit-box__texarea-actions {
		display: flex;
		/* props to animate on mount */
		display: none;
		position: absolute;
		right: 12px;
		bottom: 12px;
		align-items: center;
		gap: 10px;
		transform: translateY(10px);
		opacity: 0;
	}

	/* MODIFIERS */

	.commit-box-actions_expanded {
		display: flex;
		animation: expand-actions 0.17s ease-out forwards;
		animation-delay: 0.1s;
	}

	@keyframes expand-actions {
		from {
			transform: translateY(10px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}
</style>
