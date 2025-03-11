<script lang="ts" module>
	export interface CreatePrParams {
		title: string;
		body: string;
		draft: boolean;
	}
</script>

<script lang="ts">
	import PrDetailsModalHeader from './PrDetailsModalHeader.svelte';
	import PrTemplateSection from './PrTemplateSection.svelte';
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import { AIService } from '$lib/ai/service';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BranchStack } from '$lib/branches/branch';
	import { PatchSeries } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { parentBranch } from '$lib/branches/virtualBranchService';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { mapErrorToToast } from '$lib/forge/github/errorMap';
	import { getForge } from '$lib/forge/interface/forge';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { type DetailedPullRequest, type PullRequest } from '$lib/forge/interface/types';
	import { ReactivePRBody, ReactivePRTitle } from '$lib/forge/prContents.svelte';
	import { updatePrDescriptionTables as updatePrStackInfo } from '$lib/forge/shared/prFooter';
	import { TemplateService } from '$lib/forge/templateService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/project';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { sleep } from '$lib/utils/sleep';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import ToggleButton from '@gitbutler/ui/ToggleButton.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import { KeyName, onMetaEnter } from '@gitbutler/ui/utils/hotkeys';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { tick } from 'svelte';

	type Props =
		| {
				type: 'preview';
				currentSeries: PatchSeries;
				stackId: string;
		  }
		| {
				type: 'display';
				currentSeries: PatchSeries;
				pr: DetailedPullRequest;
		  };

	let props: Props = $props();

	const project = getContext(Project);
	const baseBranch = getContextStore(BaseBranch);
	const branchStore = getContextStore(BranchStack);
	const branchController = getContext(BranchController);
	const prService = getForgePrService();
	const aiService = getContext(AIService);
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const forge = getForge();
	const forgeListingService = getForgeListingService();
	const templateService = getContext(TemplateService);

	const stack = $derived($branchStore);
	const commits = $derived(
		props.type === 'preview'
			? props.currentSeries.patches
			: [...props.currentSeries.patches, ...props.currentSeries.upstreamPatches]
	);
	const upstreamName = $derived(
		props.type === 'preview' ? props.currentSeries.name : stack.upstreamName
	);
	const forgeBranch = $derived(upstreamName ? $forge?.branch(upstreamName) : undefined);
	const baseBranchName = $derived($baseBranch.shortName);
	const currentSeries = $derived(props.type === 'preview' ? props.currentSeries : undefined);

	let createPrDropDown = $state<ReturnType<typeof DropDownButton>>();
	const createDraft = persisted<boolean>(false, 'createDraftPr');

	let modal = $state<ReturnType<typeof Modal>>();
	let isEditing = $state<boolean>(true);
	let isLoading = $state<boolean>(false);
	let aiIsLoading = $state<boolean>(false);
	let aiConfigurationValid = $state<boolean>(false);
	let aiDescriptionDirective = $state<string | undefined>(undefined);
	let showAiBox = $state<boolean>(false);
	let templateBody = $state<string | undefined>(undefined);
	const pushBeforeCreate = $derived(!forgeBranch || commits.some((c) => !c.isRemote));

	// Displays template select component when true.
	let useTemplate = persisted(false, `use-template-${project.id}`);
	// Available pull request templates.
	let templates = $state<string[]>([]);

	const canUseAI = $derived(aiConfigurationValid && $aiGenEnabled);

	const prTitle = $derived(
		new ReactivePRTitle(
			project.id,
			props.type === 'display',
			props.type === 'display' ? props.pr.title : undefined,
			commits,
			currentSeries?.name ?? ''
		)
	);

	const prBody = $derived(
		new ReactivePRBody(
			project.id,
			props.type === 'display',
			currentSeries?.description ?? '',
			props.type === 'display' ? props.pr.body : undefined,
			commits,
			templateBody,
			currentSeries?.name ?? ''
		)
	);

	async function handleToggleUseTemplate() {
		useTemplate.set(!$useTemplate);
		if (!$useTemplate) {
			templateBody = undefined;
		}
	}

	$effect(() => {
		if (modal?.imports.open) {
			aiService.validateConfiguration().then((valid) => {
				aiConfigurationValid = valid;
			});
			if ($forge) {
				templateService.getAvailable($forge.name).then((availableTemplates) => {
					templates = availableTemplates;
				});
			}
		}
	});

	export async function createPr(params: CreatePrParams): Promise<PullRequest | undefined> {
		if (!$forge) {
			error('Pull request service not available');
			return;
		}
		if (!currentSeries) {
			return;
		}

		// All ids that existed prior to creating a new one (including archived).
		const prNumbers = stack.validSeries.map((series) => series.prNumber);

		isLoading = true;
		try {
			let upstreamBranchName = upstreamName;

			if (pushBeforeCreate) {
				const firstPush = !stack.upstream;
				const pushResult = await branchController.pushBranch(stack.id, stack.requiresForce);

				if (pushResult) {
					upstreamBranchName = getBranchNameFromRef(pushResult.refname, pushResult.remote);
				}

				if (firstPush) {
					// TODO: fix this hack for reactively available prService.
					await sleep(500);
				}
			}

			if (!baseBranchName) {
				error('No base branch name determined');
				return;
			}

			if (!upstreamBranchName) {
				error('No upstream branch name determined');
				return;
			}

			if (!$prService) {
				error('Pull request service not available');
				return;
			}

			// Find the index of the current branch so we know where we want to point the pr.
			const branches = stack.validSeries;
			const currentIndex = branches.findIndex((b) => b.name === currentSeries.name);
			if (currentIndex === -1) {
				throw new Error('Branch index not found.');
			}

			// Use base branch as base unless it's part of stack and should be be pointing
			// to the preceding branch. Ensuring we're not using `archived` branches as base.
			let base = baseBranchName;
			let parent = parentBranch(currentSeries, branches);

			if (parent && !parent.integrated && !parent.archived) {
				base = parent.branchName;
			}

			const pr = await $prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName: base,
				upstreamName: upstreamBranchName
			});

			// Store the new pull request number with the branch data.
			await branchController.updateBranchPrNumber(stack.id, currentSeries.name, pr.number);

			// If we now have two or more pull requests we add a stack table to the description.
			prNumbers[currentIndex] = pr.number;
			const definedPrNumbers = prNumbers.filter(isDefined);
			if (definedPrNumbers.length > 0) {
				updatePrStackInfo($prService, definedPrNumbers);
			}

			// Refresh store
			$forgeListingService?.refresh();
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError('Error while creating pull request', err);
		} finally {
			isLoading = false;
		}
	}

	async function handleCreatePR(close: () => void) {
		if (props.type === 'display') return;
		await createPr({
			title: prTitle.value,
			body: prBody.value,
			draft: $createDraft
		});
		close();
	}

	async function handleAIButtonPressed() {
		if (props.type === 'display') return;
		if (!aiGenEnabled) return;

		aiIsLoading = true;
		await tick();

		let firstToken = true;

		try {
			const description = await aiService?.describePR({
				title: prTitle.value,
				body: prBody.value,
				directive: aiDescriptionDirective,
				commitMessages: commits.map((c) => c.description),
				prBodyTemplate: templateBody,
				onToken: (token) => {
					if (firstToken) {
						prBody.reset();
						firstToken = false;
					}
					prBody.append(token);
				}
			});

			if (description) {
				prBody.set(description);
			}
		} finally {
			aiIsLoading = false;
			aiDescriptionDirective = undefined;
			await tick();
		}
	}

	function handleModalKeydown(e: KeyboardEvent) {
		switch (e.key) {
			case 'g':
				if ((e.metaKey || e.ctrlKey) && e.shiftKey) {
					e.stopPropagation();
					e.preventDefault();
					handleAIButtonPressed();
				}
				break;
			case KeyName.Enter:
				if (isLoading || aiIsLoading) break;
				if (e.metaKey || e.ctrlKey) {
					e.stopPropagation();
					e.preventDefault();
					handleCreatePR(() => modal?.close());
				}
				break;
		}
	}

	function showBorderOnScroll(e: Event) {
		const target = e.target as HTMLElement;
		const scrollPosition = target.scrollTop;
		const top = scrollPosition < 5;

		if (top) {
			target.style.borderTop = 'none';
		} else {
			target.style.borderTop = '1px solid var(--clr-border-3)';
		}
	}

	function onClose() {
		isEditing = true;
	}

	let prLinkCopied = $state(false);
	function handlePrLinkCopied(link: string) {
		if (!navigator.clipboard) return;

		navigator.clipboard.writeText(link);
		prLinkCopied = true;

		setTimeout(() => {
			prLinkCopied = false;
		}, 2000);
	}

	export function show() {
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};

	const isDisplay = props.type === 'display';
</script>

<Modal bind:this={modal} width={580} noPadding {onClose} onKeyDown={handleModalKeydown}>
	<!-- HEADER -->
	{#if !isDisplay}
		<PrDetailsModalHeader {isDisplay} bind:isEditing />
	{/if}

	<!-- MAIN FIELDS -->
	<ScrollableContainer wide maxHeight="66vh" onscroll={showBorderOnScroll}>
		<div class="pr-content">
			{#if isDisplay || !isEditing}
				<div class="pr-preview" class:display={isDisplay} class:preview={!isDisplay}>
					<h1 class="text-head-22 pr-preview-title">
						{prTitle.value}
					</h1>
					{#if prBody.value}
						<div class="pr-description-preview">
							<Markdown content={prBody.value} />
						</div>
					{/if}
				</div>
			{:else}
				<div class="pr-fields">
					<Textbox
						placeholder="PR title"
						value={prTitle.value}
						readonly={!isEditing || isDisplay}
						oninput={(value: string) => {
							prTitle.set(value);
						}}
					/>

					<!-- FEATURES -->
					<div class="features-section">
						<ToggleButton
							icon="doc"
							label="Use PR template"
							checked={$useTemplate}
							onclick={handleToggleUseTemplate}
							disabled={templates.length === 0}
						/>
						<ToggleButton
							icon="ai-small"
							label="AI generation"
							checked={showAiBox}
							tooltip={!aiConfigurationValid ? 'AI service is not configured' : undefined}
							disabled={!canUseAI || aiIsLoading}
							onclick={() => {
								showAiBox = !showAiBox;
							}}
						/>
					</div>

					<!-- PR TEMPLATE SELECT -->
					{#if $useTemplate}
						<PrTemplateSection
							onselected={(body) => {
								templateBody = body;
							}}
							{templates}
						/>
					{/if}

					<!-- DESCRIPTION FIELD -->
					<div class="pr-description-field text-input">
						<Textarea
							unstyled
							value={prBody.value}
							minRows={4}
							autofocus
							padding={{ top: 12, right: 12, bottom: 12, left: 12 }}
							placeholder="Add description…"
							oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
								const target = e.currentTarget as HTMLTextAreaElement;
								prBody.set(target.value);
							}}
						/>

						<!-- AI GENRATION -->
						<div class="pr-ai" class:show-ai-box={showAiBox}>
							{#if showAiBox}
								<Textarea
									unstyled
									autofocus
									bind:value={aiDescriptionDirective}
									padding={{ top: 12, right: 12, bottom: 0, left: 12 }}
									placeholder={aiService.prSummaryMainDirective}
									onkeydown={onMetaEnter(handleAIButtonPressed)}
								/>
								<div class="pr-ai__actions">
									<Button
										style="neutral"
										icon="ai-small"
										tooltip={!aiConfigurationValid
											? 'Log in or provide your own API key'
											: !$aiGenEnabled
												? 'Enable summary generation'
												: undefined}
										disabled={!canUseAI || aiIsLoading}
										loading={aiIsLoading}
										onclick={handleAIButtonPressed}
									>
										Generate description
									</Button>
								</div>
							{/if}
						</div>
					</div>
				</div>
			{/if}
		</div>
	</ScrollableContainer>

	<!-- FOOTER -->

	{#snippet controls(close)}
		{#if props.type !== 'display'}
			<Button kind="outline" onclick={close}>Cancel</Button>

			<DropDownButton
				bind:this={createPrDropDown}
				style="pop"
				disabled={isLoading || aiIsLoading || !prTitle.value}
				loading={isLoading}
				type="submit"
				onclick={async () => await handleCreatePR(close)}
			>
				{pushBeforeCreate ? 'Push & ' : ''}
				{$createDraft ? 'Create draft pull request' : `Create Pull Request`}

				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Create Pull Request"
							onclick={() => {
								createDraft.set(false);
								createPrDropDown?.close();
							}}
						/>
						<ContextMenuItem
							label="Create Draft Pull Request"
							onclick={() => {
								createDraft.set(true);
								createPrDropDown?.close();
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		{:else}
			<div class="pr-footer__actions">
				<Button
					kind="outline"
					icon={prLinkCopied ? 'tick-small' : 'copy-small'}
					disabled={prLinkCopied}
					onclick={() => {
						handlePrLinkCopied(props.pr.htmlUrl);
					}}>{prLinkCopied ? 'Link copied!' : 'Copy PR link'}</Button
				>
				<Button
					kind="outline"
					icon="open-link"
					onclick={() => {
						openExternalUrl(props.pr.htmlUrl);
					}}>Open in browser</Button
				>
			</div>
			<Button kind="outline" onclick={close}>Close</Button>
		{/if}
	{/snippet}
</Modal>

<style lang="postcss">
	.pr-content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px;
	}

	/* FIELDS */

	.pr-fields {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.pr-description-field {
		flex: 1;
		display: flex;
		flex-direction: column;
		/* reset .text-input padding */
		padding: 0;
	}

	/* PREVIEW */

	.pr-description-preview {
		overflow-y: auto;
		display: flex;
	}

	/* AI BOX */

	.pr-ai {
		display: flex;
		flex-direction: column;
	}

	.show-ai-box {
		border-top: 1px solid var(--clr-border-3);
	}

	.pr-ai__actions {
		width: 100%;
		display: flex;
		justify-content: flex-end;
		gap: 6px;
		padding: 12px;
	}

	/* FOOTER */

	.pr-footer__actions {
		width: 100%;
		display: flex;
		gap: 6px;
	}

	.features-section {
		display: flex;
		gap: 6px;
	}

	/* PREVIEW */
	.pr-preview {
		display: flex;
		flex-direction: column;
		gap: 16px;

		&.display {
			padding-top: 16px;
		}

		&.preview {
			padding: 16px;
			background-color: var(--clr-bg-1-muted);
			border-radius: var(--radius-m);
		}
	}
</style>
