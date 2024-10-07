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
	import BorderlessTextarea from '$lib/shared/BorderlessTextarea.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { User } from '$lib/stores/user';
	import { autoHeight } from '$lib/utils/autoHeight';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { KeyName, onMetaEnter } from '$lib/utils/hotkeys';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
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

	const user = getContextStore(User);
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
	// let inputTitleElem = $state<HTMLInputElement | null>(null);
	let bodyTextArea = $state<HTMLTextAreaElement | null>(null);
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
		if ($prService && pullRequestTemplateBody === undefined && prTemplatePath) {
			$prService.pullRequestTemplateContent(prTemplatePath, project.id).then((template) => {
				pullRequestTemplateBody = template;
			});
		}
	});

	$effect(() => {
		if (modal?.imports.open) {
			aiService.validateConfiguration($user?.access_token).then((valid) => {
				aiConfigurationValid = valid;
			});
		}
	});

	function updateFieldsHeight() {
		if (bodyTextArea) autoHeight(bodyTextArea);
	}

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
			userToken: $user.access_token,
			onToken: (t) => {
				if (firstToken) {
					firstToken = false;
					inputBody = '';
				}
				inputBody += t;
				updateFieldsHeight();
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

		updateFieldsHeight();
	}

	function handleModalKeydown(e: KeyboardEvent) {
		switch (e.key) {
			case 'e':
				if (e.metaKey || e.ctrlKey) {
					e.stopPropagation();
					e.preventDefault();
				}
				break;
			case 'g':
				if (e.metaKey || e.ctrlKey) {
					e.stopPropagation();
					e.preventDefault();
					handleAIButtonPressed();
				}
				break;
			case KeyName.Enter:
				if (isEditing || isLoading || aiIsLoading) break;
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

<Modal bind:this={modal} width="medium-large" noPadding {onClose} onKeyDown={handleModalKeydown}>
	<div class="pr-content">
		<!-- MAIN FIELDS -->
		<div class="pr-header">
			<div class="pr-title">
				<BorderlessTextarea
					placeholder="PR title"
					value={actualTitle}
					fontSize={18}
					readonly={!isEditing || isPreviewOnly}
					oninput={(e) => {
						inputTitle = e.currentTarget.value;
					}}
				/>
			</div>

			{#if !isPreviewOnly}
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
					<Segment id="write">Write</Segment>
					<Segment id="preview">Preview</Segment>
				</SegmentControl>
			{/if}
		</div>

		<ScrollableContainer wide maxHeight="66vh" onscroll={showBorderOnScroll}>
			{#if isPreviewOnly || !isEditing}
				<div class="pr-description-preview">
					<Markdown content={actualBody} />
				</div>
			{:else}
				<BorderlessTextarea
					bind:value={inputBody}
					padding={{ top: 0, right: 16, bottom: 16, left: 20 }}
					placeholder="Add description…"
					oninput={(e) => {
						inputBody = e.currentTarget.value;
					}}
				/>
			{/if}

			<!-- AI GENRATION -->
			{#if !isPreviewOnly && canUseAI && isEditing}
				<div class="pr-ai" class:show-ai-box={showAiBox}>
					{#if showAiBox}
						<BorderlessTextarea
							bind:value={aiDescriptionDirective}
							padding={{ top: 16, right: 16, bottom: 0, left: 20 }}
							placeholder={aiService.prSummaryMainDirective}
							onkeydown={onMetaEnter(handleAIButtonPressed)}
							oninput={(e) => {
								aiDescriptionDirective = e.currentTarget.value;
							}}
						/>
						<div class="pr-ai__actions">
							<Button style="ghost" outline onclick={() => (showAiBox = false)}>Hide</Button>
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
		</ScrollableContainer>
	</div>

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
						onclick={async () => await handleCreatePR(close)}
						>{isDraft ? 'Create draft pull request' : 'Create pull request'}</Button
					>
				</div>
			{:else}
				<div class="pr-footer__actions">
					<Button
						style="ghost"
						outline
						icon="open-link"
						onclick={() => {
							openExternalUrl(props.pr.htmlUrl);
						}}>Open in browser</Button
					>
					<Button
						style="ghost"
						outline
						icon="copy"
						onclick={() => {
							handlePrLinkCopied(props.pr.htmlUrl);
						}}>{prLinkCopied ? 'Link copied!' : 'Copy link'}</Button
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
	}

	.pr-header {
		display: flex;
		gap: 16px;
		padding: 16px 16px 12px 20px;
	}

	.pr-title {
		flex: 1;
		margin-top: 4px;
	}

	.pr-description-preview {
		overflow-y: auto;
		display: flex;
		padding: 16px 16px 16px 20px;
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
		display: flex;
		gap: 6px;
		padding: 12px 20px 16px;
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
		gap: 8px;
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
