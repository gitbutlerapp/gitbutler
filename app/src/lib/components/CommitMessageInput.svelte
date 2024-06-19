<script lang="ts">
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import {
		projectAiGenEnabled,
		projectCommitGenerationExtraConcise,
		projectCommitGenerationUseEmojis
	} from '$lib/config/config';
	import { showError } from '$lib/notifications/toasts';
	import { User } from '$lib/stores/user';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { useAutoHeight } from '$lib/utils/useAutoHeight';
	import { useResize } from '$lib/utils/useResize';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Branch, LocalFile } from '$lib/vbranches/types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { fly } from 'svelte/transition';

	export let isExpanded: boolean;
	export let commitMessage: string;
	export let valid: boolean = false;
	export let commit: (() => void) | undefined = undefined;

	const user = getContextStore(User);
	const selectedOwnership = getContextStore(Ownership);
	const aiService = getContext(AIService);
	const branch = getContextStore(Branch);
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

	let titleTextArea: HTMLTextAreaElement;
	let descriptionTextArea: HTMLTextAreaElement;

	$: ({ title, description } = splitMessage(commitMessage));
	$: valid = !!title;

	function concatMessage(title: string, description: string) {
		return `${title}\n\n${description}`;
	}

	function focusTextAreaOnMount(el: HTMLTextAreaElement) {
		el.focus();
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
			const prompt = promptService.selectedCommitPrompt(project.id);
			console.log(prompt);
			const generatedMessage = await aiService.summarizeCommit({
				hunks,
				useEmojiStyle: $commitGenerationUseEmojis,
				useBriefStyle: $commitGenerationExtraConcise,
				userToken: $user?.access_token,
				commitTemplate: prompt
			});

			if (generatedMessage) {
				commitMessage = generatedMessage;
			} else {
				throw new Error('Prompt generated no response');
			}
		} catch (e: any) {
			showError('Failed to generate commit message', e);
		} finally {
			aiLoading = false;
		}
	}

	onMount(async () => {
		aiConfigurationValid = await aiService.validateConfiguration($user?.access_token);
	});
</script>

{#if isExpanded}
	<div
		class="commit-box__textarea-wrapper text-input"
		use:useResize={() => {
			useAutoHeight(titleTextArea);
			useAutoHeight(descriptionTextArea);
		}}
	>
		<textarea
			value={title}
			placeholder="Commit summary"
			disabled={aiLoading}
			class="text-base-body-13 text-semibold commit-box__textarea commit-box__textarea__title"
			spellcheck="false"
			rows="1"
			bind:this={titleTextArea}
			use:focusTextAreaOnMount
			on:focus={(e) => useAutoHeight(e.currentTarget)}
			on:input={(e) => {
				commitMessage = concatMessage(e.currentTarget.value, description);
				useAutoHeight(e.currentTarget);
			}}
			on:keydown={(e) => {
				if (commit && (e.ctrlKey || e.metaKey) && e.key === 'Enter') commit();
				if (e.key === 'Enter') {
					e.preventDefault();
					descriptionTextArea.focus();
				}
			}}
		></textarea>

		{#if title.length > 0 || description}
			<textarea
				value={description}
				disabled={aiLoading}
				placeholder="Commit description (optional)"
				class="text-base-body-13 commit-box__textarea commit-box__textarea__description"
				spellcheck="false"
				rows="1"
				bind:this={descriptionTextArea}
				on:focus={(e) => useAutoHeight(e.currentTarget)}
				on:input={(e) => {
					commitMessage = concatMessage(title, e.currentTarget.value);
					useAutoHeight(e.currentTarget);
				}}
				on:keydown={(e) => {
					const value = e.currentTarget.value;
					if (e.key === 'Backspace' && value.length === 0) {
						e.preventDefault();
						titleTextArea.focus();
						useAutoHeight(e.currentTarget);
					} else if (e.key === 'a' && (e.metaKey || e.ctrlKey) && value.length === 0) {
						// select previous textarea on cmd+a if this textarea is empty
						e.preventDefault();
						titleTextArea.select();
					}
				}}
			></textarea>
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
			class:commit-box-actions_expanded={isExpanded}
			use:tooltip={$aiGenEnabled && aiConfigurationValid
				? ''
				: 'You must be logged in or have provided your own API key and have summary generation enabled to use this feature'}
		>
			<DropDownButton
				style="ghost"
				outline
				icon="ai-small"
				disabled={!($aiGenEnabled && aiConfigurationValid)}
				loading={aiLoading}
				menuPosition="top"
				on:click={async () => await generateCommitMessage($branch.files)}
			>
				Generate message
				<ContextMenu slot="context-menu">
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
{/if}

<style lang="postcss">
	.commit-box__textarea-wrapper {
		display: flex;
		position: relative;
		padding: 0 0 48px;
		flex-direction: column;
		gap: 4px;
		overflow: hidden;
		animation: expand-box 0.2s ease forwards;
		/* props to animate on mount */
		/* display: none;
		max-height: 0;
		overflow: hidden; */
	}

	.commit-box__textarea {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 16px;
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
		bottom: 12px;
		left: 12px;
		padding: 2px;
		border-radius: 100%;
		background: var(--clr-bg-2);
		color: var(--clr-scale-ntrl-40);
	}

	.commit-box__textarea__title {
		min-height: 31px;
		padding: 12px 12px 0 12px;
	}

	.commit-box__textarea__description {
		padding: 0 12px 0 12px;
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
	/* .commit-box_expanded {
		display: flex;
		animation: expand-box 0.2s ease forwards;
	} */

	@keyframes expand-box {
		from {
			max-height: 0;
			padding: 0 0 0;
		}
		to {
			max-height: 600px;
			padding: 0 0 48px;
		}
	}

	.commit-box-actions_expanded {
		display: flex;
		animation: expand-actions 0.25s ease forwards;
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
