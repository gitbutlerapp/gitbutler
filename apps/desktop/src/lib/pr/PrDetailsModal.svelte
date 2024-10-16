<script lang="ts" module>
	export interface CreatePrParams {
		title: string;
		body: string;
		draft: boolean;
	}
</script>

<script lang="ts">
	import { getPreferredPRAction, PRAction } from './pr';
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import Markdown from '$lib/components/Markdown.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { mapErrorToToast } from '$lib/gitHost/github/errorMap';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { isFailure } from '$lib/result';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { KeyName, onMetaEnter } from '$lib/utils/hotkeys';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import BorderlessTextarea from '@gitbutler/ui/BorderlessTextarea.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';
	import { tick } from 'svelte';
	import type { DetailedPullRequest, PullRequest } from '$lib/gitHost/interface/types';

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
		name: string;
		upstreamName?: string;
		commits: DetailedCommit[];
	}

	type Props = DisplayProps | PreviewProps | PreviewSeriesProps;

	let props: Props = $props();

	const project = getContext(Project);
	const baseBranch = getContextStore(BaseBranch);
	const branchStore = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const baseBranchService = getContext(BaseBranchService);
	const gitListService = getGitHostListingService();
	const prService = getGitHostPrService();
	const aiService = getContext(AIService);
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const gitHost = getGitHost();
	const preferredPRAction = getPreferredPRAction();

	const branch = $derived($branchStore);
	const branchName = $derived(props.type === 'preview-series' ? props.name : branch.name);
	const commits = $derived(props.type === 'preview-series' ? props.commits : branch.commits);
	const upstreamName = $derived(
		props.type === 'preview-series' ? props.upstreamName : branch.upstreamName
	);
	const baseBranchName = $derived($baseBranch.shortName);
	const prTemplatePath = $derived(project.git_host.pullRequestTemplatePath);
	let isDraft = $state<boolean>($preferredPRAction === PRAction.CreateDraft);

	let modal = $state<ReturnType<typeof Modal>>();
	let isEditing = $state<boolean>(true);
	let isLoading = $state<boolean>(false);
	let pullRequestTemplateBody = $state<string | undefined>(undefined);
	let aiIsLoading = $state<boolean>(false);
	let aiConfigurationValid = $state<boolean>(false);
	let aiDescriptionDirective = $state<string | undefined>(undefined);
	let showAiBox = $state<boolean>(false);

	const canUseAI = $derived.by(() => {
		return aiConfigurationValid || $aiGenEnabled;
	});
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
		if (pullRequestTemplateBody) return pullRequestTemplateBody;
		// In case of a single commit, use the commit description for the body
		if (commits.length === 1) {
			const commit = commits[0];
			return commit?.descriptionBody ?? '';
		} else {
			return '';
		}
	});

	let inputBody = $state<string | undefined>(undefined);
	let inputTitle = $state<string | undefined>(undefined);
	const actualBody = $derived<string>(inputBody ?? defaultBody);
	const actualTitle = $derived<string>(inputTitle ?? defaultTitle);

	// Fetch PR template content
	$effect(() => {
		if (
			modal?.imports.open &&
			$prService &&
			pullRequestTemplateBody === undefined &&
			prTemplatePath
		) {
			$prService.pullRequestTemplateContent(prTemplatePath, project.id).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	$effect(() => {
		if (modal?.imports.open) {
			aiService.validateConfiguration().then((valid) => {
				aiConfigurationValid = valid;
			});
		}
	});

	async function createPr(params: CreatePrParams): Promise<PullRequest | undefined> {
		if (!$gitHost) {
			error('Pull request service not available');
			return;
		}

		isLoading = true;
		try {
			let upstreamBranchName = upstreamName;

			if (commits.some((c) => !c.isRemote)) {
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

			await $prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName,
				upstreamName: upstreamBranchName
			});
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError('Error while creating pull request', err);
		} finally {
			isLoading = false;
		}
		await $gitListService?.refresh();
		baseBranchService.fetchFromRemotes();
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

	function handleCheckDraft() {
		isDraft = !isDraft;
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
			prBodyTemplate: pullRequestTemplateBody,
			onToken: async (t) => {
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
		inputTitle = undefined;
		inputBody = undefined;
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

	const isPreviewOnly = props.type === 'display';
</script>

<Modal bind:this={modal} width={580} noPadding {onClose} onKeyDown={handleModalKeydown}>
	<div class="pr-header">
		{#if !isPreviewOnly}
			<h3 class="text-14 text-semibold pr-title">
				{!isEditing ? actualTitle : 'Create a pull request'}
			</h3>
			<SegmentControl
				defaultIndex={isPreviewOnly ? 1 : 0}
				onselect={(id) => {
					if (id === 'write') {
						isEditing = true;
					} else {
						isEditing = false;
					}
				}}
			>
				<Segment unfocusable id="write">Edit</Segment>
				<Segment unfocusable id="preview">Preview</Segment>
			</SegmentControl>
		{:else}
			<h3 class="text-14 text-semibold pr-title">{actualTitle}</h3>
		{/if}
	</div>

	<!-- HEADER -->

	<!-- MAIN FIELDS -->
	<ScrollableContainer wide maxHeight="66vh" onscroll={showBorderOnScroll}>
		<div class="pr-content">
			{#if isPreviewOnly || !isEditing}
				<div class="pr-description-preview">
					<Markdown content={actualBody} />
				</div>
			{:else}
				<div class="pr-fields">
					<TextBox
						placeholder="PR title"
						value={actualTitle}
						readonly={!isEditing || isPreviewOnly}
						on:input={(e) => {
							inputTitle = e.detail;
						}}
					/>

					<!-- DESCRIPTION FIELD -->
					<div class="pr-description-field text-input">
						<BorderlessTextarea
							value={actualBody}
							autofocus
							padding={{ top: 12, right: 12, bottom: 0, left: 12 }}
							placeholder="Add description…"
							oninput={(e: InputEvent) => {
								const target = e.target as HTMLTextAreaElement;
								inputBody = target.value;
							}}
						/>

						<!-- AI GENRATION -->
						{#if !isPreviewOnly && canUseAI && isEditing}
							<div class="pr-ai" class:show-ai-box={showAiBox}>
								{#if showAiBox}
									<BorderlessTextarea
										autofocus
										bind:value={aiDescriptionDirective}
										padding={{ top: 12, right: 12, bottom: 0, left: 12 }}
										placeholder={aiService.prSummaryMainDirective}
										onkeydown={onMetaEnter(handleAIButtonPressed)}
										oninput={(e: InputEvent) => {
											const target = e.target as HTMLTextAreaElement;
											aiDescriptionDirective = target.value;
										}}
									/>
									<div class="pr-ai__actions">
										<Button
											style="ghost"
											outline
											onclick={() => {
												showAiBox = false;
												aiDescriptionDirective = undefined;
											}}>Hide</Button
										>
										<Button
											style="neutral"
											kind="solid"
											icon="ai-small"
											tooltip={!aiConfigurationValid
												? 'You must be logged in or have provided your own API key'
												: !$aiGenEnabled
													? 'You must have summary generation enabled'
													: undefined}
											disabled={!canUseAI || aiIsLoading}
											isLoading={aiIsLoading}
											onclick={handleAIButtonPressed}
										>
											Generate
										</Button>
									</div>
								{:else}
									<div class="pr-ai__actions">
										<Button
											style="ghost"
											outline
											icon="ai-small"
											tooltip={!aiConfigurationValid
												? 'You must be logged in or have provided your own API key'
												: !$aiGenEnabled
													? 'You must have summary generation enabled'
													: undefined}
											disabled={!canUseAI || aiIsLoading}
											isLoading={aiIsLoading}
											onclick={() => {
												showAiBox = true;
											}}
										>
											Generate description
										</Button>
									</div>
								{/if}
							</div>
						{/if}
					</div>
				</div>
			{/if}
		</div>
	</ScrollableContainer>

	<!-- FOOTER -->

	{#snippet controls(close)}
		<div class="pr-footer">
			{#if props.type !== 'display'}
				<label class="draft-toggle__wrap">
					<Toggle id="is-draft-toggle" small checked={isDraft} on:click={handleCheckDraft} />
					<label class="text-12 draft-toggle__label" for="is-draft-toggle">Create as a draft</label>
				</label>

				<div class="pr-footer__actions">
					<Button style="ghost" outline onclick={close}>Cancel</Button>
					<Button
						style="pop"
						kind="solid"
						disabled={isLoading || aiIsLoading}
						{isLoading}
						type="submit"
						onclick={async () => await handleCreatePR(close)}
						>{isDraft ? 'Create draft pull request' : 'Create pull request'}</Button
					>
				</div>
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
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.pr-content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px;
	}

	.pr-header {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 16px 16px 14px;
	}

	/* FIELDS */

	.pr-fields {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.pr-description-field {
		flex: 1;
		display: flex;
		flex-direction: column;
		/* reset .text-input padding */
		padding: 0;
	}

	/* PREVIEW */

	.pr-title {
		flex: 1;
		margin-top: 4px;
	}

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
		margin-top: 12px;
		border-top: 1px solid var(--clr-border-3);
	}

	.pr-ai__actions {
		display: flex;
		gap: 6px;
		padding: 12px;
	}

	/* FOOTER */

	.pr-footer {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
	}

	.pr-footer__actions {
		display: flex;
		gap: 6px;
	}

	.draft-toggle__wrap {
		display: flex;
		align-items: center;
		gap: 10px;
	}

	.draft-toggle__label {
		color: var(--clr-text-2);
	}
</style>
