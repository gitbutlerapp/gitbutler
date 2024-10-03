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
	import { User } from '$lib/stores/user';
	import { autoHeight } from '$lib/utils/autoHeight';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { onMetaEnter } from '$lib/utils/hotkeys';
	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { tick } from 'svelte';
	import type { DetailedPullRequest, PullRequest } from '$lib/gitHost/interface/types';

	interface BaseProps {
		type: 'display' | 'preview';
	}

	interface DisplayProps extends BaseProps {
		type: 'display';
		pr: DetailedPullRequest;
	}

	interface PreviewProps extends BaseProps {
		type: 'preview';
	}

	type Props = DisplayProps | PreviewProps;

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
	const baseBranchName = $derived($baseBranch.shortName);
	const prTemplatePath = $derived(project.git_host.pullRequestTemplatePath);
	const isDraft = $derived<boolean>($preferredPRAction === PRAction.CreateDraft);

	let modal = $state<Modal>();
	let bodyTextArea = $state<HTMLTextAreaElement | null>(null);
	let isEditing = $state<boolean>(false);
	let isLoading = $state<boolean>(false);
	let pullRequestTemplateBody = $state<string | undefined>(undefined);
	let aiIsLoading = $state<boolean>(false);
	let aiConfigurationValid = $state<boolean>(false);
	let aiDescriptionDirective = $state<string | undefined>(undefined);

	const canUseAI = $derived.by(() => {
		return aiConfigurationValid || $aiGenEnabled;
	});
	const defaultTitle: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.title;
		// In case of a single commit, use the commit summary for the title
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
			return commit?.descriptionTitle ?? '';
		} else {
			return branch.name;
		}
	});

	const defaultBody: string = $derived.by(() => {
		if (props.type === 'display') return props.pr.body ?? '';
		if (pullRequestTemplateBody) return pullRequestTemplateBody;
		// In case of a single commit, use the commit description for the body
		if (branch.commits.length === 1) {
			const commit = branch.commits[0];
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
			let upstreamBranchName = branch.upstreamName;

			if (branch.commits.some((c) => !c.isRemote)) {
				const firstPush = !branch.upstream;
				const { refname, remote } = await branchController.pushBranch(
					branch.id,
					branch.requiresForce
				);
				upstreamBranchName = getBranchNameFromRef(refname, remote);

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

	function handleCreatePR(close: () => void) {
		if (props.type !== 'preview') return;
		createPr({
			title: actualTitle,
			body: actualBody,
			draft: isDraft
		});
		close();
	}

	function handleCheckDraft(
		e: Event & {
			currentTarget: EventTarget & HTMLInputElement;
		}
	) {
		const isDraft = e.currentTarget.checked;
		preferredPRAction.set(isDraft ? PRAction.CreateDraft : PRAction.Create);
	}

	function toggleEdit() {
		isEditing = !isEditing;
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
			commitMessages: branch.commits.map((c) => c.description),
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

	function onClose() {
		isEditing = false;
		inputTitle = undefined;
		inputBody = undefined;
	}

	export function show() {
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};
</script>

<Modal bind:this={modal} width="large" noPadding {onClose}>
	{#snippet children(_, close)}
		<ScrollableContainer maxHeight="70vh">
			<div class="pr-modal__content">
				<div class="card">
					<div
						class="card__header text-14 text-body text-semibold pr-modal__header"
						class:editing={isEditing}
					>
						{#if isEditing}
							<div class="text-input pr-modal__title-input-wrapper">
								<input
									tabindex="0"
									type="text"
									class="text-13 text-body pr-modal__title-input"
									value={actualTitle}
									oninput={(e) => {
										inputTitle = e.currentTarget.value;
									}}
								/>
							</div>
						{:else if actualTitle}
							{actualTitle}
						{:else}
							<span class="text-clr2"> No title provided.</span>
						{/if}
					</div>
					{#if isEditing}
						<div
							class="pr-modal__body-input-wrapper text-input"
							use:resizeObserver={updateFieldsHeight}
						>
							<textarea
								tabindex="0"
								bind:this={bodyTextArea}
								disabled={aiIsLoading}
								value={actualBody}
								onfocus={(e) => autoHeight(e.currentTarget)}
								oninput={(e) => {
									inputBody = e.currentTarget.value;
									autoHeight(e.currentTarget);
								}}
								class="text-13 text-body pr-modal__body-input"
							></textarea>
						</div>
					{:else if actualBody}
						<div class="card__content text-13 text-body">
							<Markdown content={actualBody} />
						</div>
					{:else}
						<div class="card__content text-13 text-body text-clr2">No PR description.</div>
					{/if}
				</div>
			</div>
		</ScrollableContainer>
		<div class="pr-modal__footer">
			{#if isEditing && canUseAI}
				<div class="text-input pr-modal__ai-prompt-wrapper">
					<textarea
						tabindex="0"
						class="text-13 text-body pr-modal__ai-prompt-input"
						disabled={aiIsLoading}
						value={aiDescriptionDirective ?? ''}
						placeholder={aiService.prSummaryMainDirective}
						onkeydown={onMetaEnter(handleAIButtonPressed)}
						onfocus={(e) => autoHeight(e.currentTarget)}
						oninput={(e) => {
							aiDescriptionDirective = e.currentTarget.value;
							autoHeight(e.currentTarget);
						}}
					></textarea>
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
						onclick={handleAIButtonPressed}>Generate description</Button
					>
				</div>
			{/if}
			<div class="pr-modal__button-wrapper">
				{#if props.type === 'preview'}
					<div class="pr-modal__checkbox-wrapper">
						<Checkbox name="is-draft" small checked={isDraft} onchange={handleCheckDraft} />
						<label class="text-13" for="is-draft">Draft</label>
					</div>

					<Button style="ghost" outline onclick={close}>Cancel</Button>
					<Button style="neutral" kind="solid" onclick={toggleEdit}
						>{isEditing ? 'Done' : 'Edit'}</Button
					>
					<Button
						style="pop"
						kind="solid"
						disabled={isEditing || isLoading}
						{isLoading}
						onclick={() => handleCreatePR(close)}
						>{isDraft ? 'Create Draft PR' : 'Create PR'}</Button
					>
				{:else if props.type === 'display'}
					<Button style="ghost" outline onclick={close}>Done</Button>
				{/if}
			</div>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.pr-modal__content {
		padding: 16px;
	}

	.pr-modal__header {
		position: sticky;
		top: 0;
		background: var(--clr-bg-1);
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
		&.editing {
			padding: 8px;
		}
	}

	.pr-modal__title-input-wrapper {
		display: flex;
		position: relative;
		width: 100%;
		flex-direction: column;
		gap: 4px;
	}
	.pr-modal__title-input {
		width: 100%;
		border: none;
		background: none;
		outline: none;
	}

	.pr-modal__body-input-wrapper {
		display: flex;
		position: relative;
		padding: 16px;
		margin: 8px;
		flex-direction: column;
		gap: 4px;
	}
	.pr-modal__ai-prompt-input,
	.pr-modal__body-input {
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

	.pr-modal__ai-prompt-input {
		width: 100%;
	}

	.pr-modal__footer {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 16px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		border-bottom-left-radius: var(--radius-l);
		border-bottom-right-radius: var(--radius-l);
	}

	.pr-modal__checkbox-wrapper {
		display: flex;
		width: 100%;
		gap: 8px;
		justify-content: flex-start;
		align-items: center;
	}

	.pr-modal__button-wrapper {
		display: flex;
		gap: 8px;
		width: 100%;
		justify-content: flex-end;
		align-items: center;
	}

	.pr-modal__ai-prompt-wrapper {
		display: flex;
		width: 100%;
		padding: 8px;
	}
</style>
