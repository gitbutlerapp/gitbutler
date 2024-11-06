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
	import { getPreferredPRAction, PRAction } from './pr';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import { TemplateService } from '$lib/backend/templateService';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import Markdown from '$lib/components/Markdown.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { mapErrorToToast } from '$lib/forge/github/errorMap';
	import { getForge } from '$lib/forge/interface/forge';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { isFailure } from '$lib/result';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { KeyName, onMetaEnter } from '$lib/utils/hotkeys';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { PatchSeries, VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import ToggleButton from '@gitbutler/ui/ToggleButton.svelte';
	import { tick } from 'svelte';
	import type { DetailedPullRequest, PullRequest } from '$lib/forge/interface/types';

	interface BaseProps {
		type: 'display' | 'preview' | 'preview-series';
	}

	interface DisplayProps extends BaseProps {
		type: 'display';
		pr: DetailedPullRequest;
	}

	interface PreviewProps extends BaseProps {
		type: 'preview';
	}

	interface PreviewSeriesProps {
		type: 'preview-series';
		currentSeries: PatchSeries;
		stackId: string;
	}

	type Props = DisplayProps | PreviewProps | PreviewSeriesProps;

	let props: Props = $props();

	const project = getContext(Project);
	const baseBranch = getContextStore(BaseBranch);
	const branchStore = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const prService = getForgePrService();
	const aiService = getContext(AIService);
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const forge = getForge();
	const templateService = getContext(TemplateService);
	const preferredPRAction = getPreferredPRAction();

	const branch = $derived($branchStore);
	const branchName = $derived(
		props.type === 'preview-series' ? props.currentSeries.name : branch.name
	);
	const commits = $derived(
		props.type === 'preview-series' ? props.currentSeries.patches : branch.commits
	);
	const upstreamName = $derived(
		props.type === 'preview-series' ? props.currentSeries.name : branch.upstreamName
	);
	const baseBranchName = $derived($baseBranch.shortName);

	let createPrDropDown = $state<ReturnType<typeof DropDownButton>>();
	let isDraft = $state<boolean>($preferredPRAction === PRAction.CreateDraft);

	let modal = $state<ReturnType<typeof Modal>>();
	let templateSelector = $state<ReturnType<typeof PrTemplateSection>>();
	let isEditing = $state<boolean>(true);
	let isLoading = $state<boolean>(false);
	let templateBody = $state<string | undefined>(undefined);
	let aiIsLoading = $state<boolean>(false);
	let aiConfigurationValid = $state<boolean>(false);
	let aiDescriptionDirective = $state<string | undefined>(undefined);
	let showAiBox = $state<boolean>(false);
	let pushBeforeCreate = $state(false);

	// Displays template select component when true.
	let useTemplate = persisted(false, `use-template-${project.id}`);
	// Available pull request templates.
	let templates = $state<string[]>([]);

	async function handleToggleUseTemplate() {
		useTemplate.set(!$useTemplate);
		if (!$useTemplate) {
			templateBody = undefined;
		}
	}

	const canUseAI = $derived(aiConfigurationValid && $aiGenEnabled);
	const defaultTitle: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.title;

		// In case of a single commit, use the commit summary for the title
		if (commits.length === 1) {
			const commit = commits[0];
			return commit?.descriptionTitle ?? '';
		} else {
			return branchName;
		}
	});

	const defaultBody: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.body ?? '';
		if (props.type === 'preview-series' && props.currentSeries.description)
			return props.currentSeries.description;
		if (templateBody) return templateBody;

		// In case of a single commit, use the commit description for the body
		if (commits.length === 1) {
			const commit = commits[0];
			return commit?.descriptionBody ?? '';
		} else {
			return '';
		}
	});

	let inputBody = $state<string>('');
	let inputTitle = $state<string>('');
	const actualBody = $derived<string>(inputBody.trim().length > 0 ? inputBody : defaultBody);
	const actualTitle = $derived<string>(inputTitle.trim().length > 0 ? inputTitle : defaultTitle);

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

		isLoading = true;
		try {
			let upstreamBranchName = upstreamName;

			if (pushBeforeCreate || commits.some((c) => !c.isRemote)) {
				const firstPush = !branch.upstream;
				const pushResult = await branchController.pushBranch(
					branch.id,
					branch.requiresForce,
					props.type === 'preview-series'
				);

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

			const pr = await $prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName,
				upstreamName: upstreamBranchName
			});
			if (props.type === 'preview-series') {
				await branchController.updateSeriesPrNumber(
					props.stackId,
					props.currentSeries.name,
					pr.number
				);
			}
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
			title: actualTitle,
			body: actualBody,
			draft: isDraft
		});
		close();
	}

	async function handleAIButtonPressed() {
		if (props.type === 'display') return;
		if (!aiGenEnabled) return;

		aiIsLoading = true;
		await tick();

		let firstToken = true;

		const descriptionResult = await aiService?.describePR({
			title: actualTitle,
			body: actualBody,
			directive: aiDescriptionDirective,
			commitMessages: commits.map((c) => c.description),
			prBodyTemplate: templateBody,
			onToken: (t) => {
				if (firstToken) {
					inputBody = '';
					firstToken = false;
				}
				inputBody += t;
			}
		});

		if (isFailure(descriptionResult)) {
			showError('Failed to generate commit message', descriptionResult.failure);
			aiIsLoading = false;
			return;
		}

		inputBody = descriptionResult.value;
		aiIsLoading = false;
		aiDescriptionDirective = undefined;
		await tick();
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
		inputTitle = '';
		inputBody = '';
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

	/**
	 * @param {boolean} pushAndCreate - Whether or not the commits need pushed before opening a PR
	 */
	export function show(pushAndCreate = false) {
		pushBeforeCreate = pushAndCreate;
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
						{actualTitle}
					</h1>
					{#if actualBody}
						<div class="pr-description-preview">
							<Markdown content={actualBody} />
						</div>
					{/if}
				</div>
			{:else}
				<div class="pr-fields">
					<Textbox
						placeholder="PR title"
						value={actualTitle}
						readonly={!isEditing || isDisplay}
						oninput={(value: string) => {
							inputTitle = value;
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
							bind:this={templateSelector}
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
							value={actualBody}
							minRows={4}
							autofocus
							padding={{ top: 12, right: 12, bottom: 12, left: 12 }}
							placeholder="Add description…"
							onchange={(e: InputEvent) => {
								const target = e.currentTarget as HTMLTextAreaElement;
								inputBody = target.value.trim();
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
										kind="solid"
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
			<Button style="ghost" outline onclick={close}>Cancel</Button>

			<DropDownButton
				bind:this={createPrDropDown}
				style="pop"
				kind="solid"
				disabled={isLoading || aiIsLoading || !actualTitle}
				loading={isLoading}
				type="submit"
				onclick={async () => await handleCreatePR(close)}
			>
				{pushBeforeCreate ? 'Push and ' : ''}
				{isDraft ? 'Create pull request draft' : `Create pull request`}

				{#snippet contextMenuSlot()}
					<ContextMenuSection>
						<ContextMenuItem
							label="Create pull request"
							onclick={() => {
								isDraft = false;
								createPrDropDown?.close();
							}}
						/>
						<ContextMenuItem
							label="Create draft pull request"
							onclick={() => {
								isDraft = true;
								createPrDropDown?.close();
							}}
						/>
					</ContextMenuSection>
				{/snippet}
			</DropDownButton>
		{:else}
			<div class="pr-footer__actions">
				<Button
					style="ghost"
					outline
					icon={prLinkCopied ? 'tick-small' : 'copy-small'}
					disabled={prLinkCopied}
					onclick={() => {
						handlePrLinkCopied(props.pr.htmlUrl);
					}}>{prLinkCopied ? 'Link copied!' : 'Copy PR link'}</Button
				>
				<Button
					style="ghost"
					outline
					icon="open-link"
					onclick={() => {
						openExternalUrl(props.pr.htmlUrl);
					}}>Open in browser</Button
				>
			</div>
			<Button style="ghost" outline onclick={close}>Close</Button>
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
