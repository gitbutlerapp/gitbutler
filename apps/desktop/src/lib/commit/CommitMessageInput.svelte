<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { AIService, type DiffInput } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis,
		projectRunCommitHooks
	} from '$lib/config/config';
	import { HooksService } from '$lib/hooks/hooksService';
	import { showError } from '$lib/notifications/toasts';
	import { isFailure } from '$lib/result';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { KeyName } from '$lib/utils/hotkeys';
	import * as toasts from '$lib/utils/toasts';
	import { SelectedOwnership } from '$lib/vbranches/ownership';
	import { listCommitFiles } from '$lib/vbranches/remoteCommits';
	import { BranchStack, DetailedCommit, Commit } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { onMount } from 'svelte';

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

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const commitGenerationExtraConcise = projectCommitGenerationExtraConcise(project.id);
	const commitGenerationUseEmojis = projectCommitGenerationUseEmojis(project.id);
	const runHooks = projectRunCommitHooks(project.id);

	let aiLoading = $state(false);
	let hookRunning = $state(false);
	let aiConfigurationValid = $state(false);

	let messageTextArea: HTMLTextAreaElement | undefined = $state();
	let isMessageFocused = $state(false);

	$effect(() => {
		valid = !!commitMessage;
	});

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

		const files = await listCommitFiles(project.id, existingCommit.id);
		return files.flatMap((file) =>
			file.hunks.map((hunk) => ({
				filePath: file.path,
				diff: hunk.diff
			}))
		);
	}

	async function generateCommitMessage() {
		const diffInput = await getDiffInput();

		aiLoading = true;

		const prompt = promptService.selectedCommitPrompt(project.id);

		let firstToken = true;

		const generatedMessageResult = await aiService.summarizeCommit({
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

		if (isFailure(generatedMessageResult)) {
			showError('Failed to generate commit message', generatedMessageResult.failure);
			aiLoading = false;
			return;
		}

		const generatedMessage = generatedMessageResult.value;

		if (generatedMessage) {
			commitMessage = generatedMessage;
		} else {
			showError('Failed to generate commit message', 'Prompt returned no response');
			aiLoading = false;
			return;
		}

		aiLoading = false;
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

	function handleKeyDown(e: KeyboardEvent & { currentTarget: HTMLTextAreaElement }) {
		if (e.key === KeyName.Escape) {
			e.preventDefault();
			cancel();
			return;
		}

		if (commit && (e.ctrlKey || e.metaKey) && e.key === KeyName.Enter) commit();
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
			value={commitMessage}
			unstyled
			placeholder="Commit message (required)"
			disabled={aiLoading}
			fontSize={13}
			padding={{ top: 12, right: 12, bottom: 0, left: 12 }}
			spellcheck={false}
			minRows={1}
			maxRows={30}
			bind:textBoxEl={messageTextArea}
			onfocus={() => {
				isMessageFocused = true;
			}}
			onblur={() => {
				isMessageFocused = false;
				if ($runHooks) {
					runMessageHook();
				}
			}}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				commitMessage = target.value;
			}}
			onkeydown={handleKeyDown}
		/>

		{#if commitMessage.length > 0 && isMessageFocused}
			{@render charCounter('Message chars', commitMessage.length)}
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
					style="ghost"
					outline
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

	.commit-box__textarea-char-counter-label {
		position: absolute;
		top: 2px;
		right: 4px;
		color: var(--clr-text-3);
		padding: 4px;
	}

	.commit-box__texarea-actions {
		display: flex;
		align-items: center;
		gap: 10px;
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
