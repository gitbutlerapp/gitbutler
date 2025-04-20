<script lang="ts" module>
	export interface CreatePrParams {
		title: string;
		body: string;
		draft: boolean;
		upstreamBranchName: string | undefined;
	}
</script>

<script lang="ts">
	import PrTemplateSection from '$components/PrTemplateSection.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import { AIService } from '$lib/ai/service';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { ButRequestDetailsService } from '$lib/forge/butRequestDetailsService';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { mapErrorToToast } from '$lib/forge/github/errorMap';
	import { type PullRequest } from '$lib/forge/interface/types';
	import { ReactivePRBody, ReactivePRTitle } from '$lib/forge/prContents.svelte';
	import {
		BrToPrService,
		updatePrDescriptionTables as updatePrStackInfo
	} from '$lib/forge/shared/prFooter';
	import { TemplateService } from '$lib/forge/templateService';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { ProjectsService } from '$lib/project/projectsService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { sleep } from '$lib/utils/sleep';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		onClose: () => void;
	};

	const { projectId, stackId, branchName, onClose }: Props = $props();

	const baseBranch = getContext(BaseBranch);
	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);
	const stackPublishingService = getContext(StackPublishingService);
	const butRequestDetailsService = getContext(ButRequestDetailsService);
	const brToPrService = getContext(BrToPrService);
	const posthog = getContext(PostHogWrapper);
	const stackService = getContext(StackService);
	const projectsService = getContext(ProjectsService);
	const userService = getContext(UserService);
	const templateService = getContext(TemplateService);
	const aiService = getContext(AIService);

	const user = userService.user;
	const project = projectsService.getProjectStore(projectId);

	const [publishBranch, branchPublishing] = stackService.publishBranch;
	const [updateBranchPrNumber, PRNumberUpdate] = stackService.updateBranchPrNumber;
	const [pushStack, stackPush] = stackService.pushStack;

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branch = $derived(branchResult.current.data);
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branches = $derived(branchesResult.current.data || []);
	const branchParentResult = $derived(
		stackService.branchParentByName(projectId, stackId, branchName)
	);
	const branchParent = $derived(branchParentResult.current.data);
	const branchParentDetailsResult = $derived(
		branchParent ? stackService.branchDetails(projectId, stackId, branchParent.name) : undefined
	);
	const branchParentDetails = $derived(branchParentDetailsResult?.current.data);
	const branchDetailsResult = $derived(
		branchParent ? stackService.branchDetails(projectId, stackId, branchName) : undefined
	);
	const branchDetails = $derived(branchDetailsResult?.current.data);
	const commitsResult = $derived(stackService.commits(projectId, stackId, branchName));
	const commits = $derived(commitsResult.current.data || []);

	const canPublish = stackPublishingService.canPublish;
	const prNumber = $derived(branch?.prNumber ?? undefined);

	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const forgeBranch = $derived(branch?.name ? forge.current.branch(branch?.name) : undefined);
	const baseBranchName = $derived(baseBranch.shortName);

	const createDraft = persisted<boolean>(false, 'createDraftPr');
	const createButlerRequest = persisted<boolean>(false, 'createButlerRequest');
	const createPullRequest = persisted<boolean>(true, 'createPullRequest');

	const pushBeforeCreate = $derived(
		!forgeBranch || commits.some((c) => c.state.type === 'LocalOnly')
	);

	let titleInput = $state<ReturnType<typeof Textbox>>();

	// Available pull request templates.
	let templates = $state<string[]>([]);

	// Load the available templates when the component is mounted.
	$effect(() => {
		templateService.getAvailable(forge.current.name).then((templatesResponse) => {
			templates = templatesResponse;
		});
	});

	// AI things
	const aiGenEnabled = projectAiGenEnabled(projectId);
	let aiConfigurationValid = $state(false);
	const canUseAI = $derived($aiGenEnabled && aiConfigurationValid);
	let aiIsLoading = $state(false);

	$effect(() => {
		aiService.validateConfiguration().then((valid) => {
			aiConfigurationValid = valid;
		});
	});

	const isExecuting = $derived(
		branchPublishing.current.isLoading ||
			PRNumberUpdate.current.isLoading ||
			stackPush.current.isLoading ||
			aiIsLoading
	);

	const canPublishBR = $derived(!!($canPublish && branch?.name && !branch.reviewId));
	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));

	const prTitle = $derived(new ReactivePRTitle(projectId, commits, branch?.name ?? ''));

	const prBody = new ReactivePRBody();

	$effect(() => {
		prBody.init(projectId, branch?.description ?? '', commits, branch?.name ?? '');
	});

	async function pushIfNeeded(): Promise<string | undefined> {
		let upstreamBranchName: string | undefined = branch?.name;
		if (pushBeforeCreate) {
			const firstPush = branchDetails?.pushStatus === 'completelyUnpushed';
			const pushResult = await pushStack({
				projectId,
				stackId,
				withForce: branchDetails?.pushStatus === 'unpushedCommitsRequiringForce'
			});

			if (pushResult) {
				upstreamBranchName = getBranchNameFromRef(pushResult.refname, pushResult.remote);
			}

			if (firstPush) {
				// TODO: fix this hack for reactively available prService.
				await sleep(500);
			}
		}

		return upstreamBranchName;
	}

	function shouldAddPrBody() {
		// If there is a branch review already, then the BR to PR sync will
		// update the PR description for us.
		if (branch?.reviewId) return false;
		// If we can't publish a BR, then we must add the PR description
		if (!canPublishBR) return true;
		// If the user wants to create a butler request then we don't want
		// to add the PR body as it will be handled by the syncing
		return !$createButlerRequest;
	}

	export async function createReview() {
		if (isExecuting) return;
		if (!branch) return;
		if (!$user) return;

		const upstreamBranchName = await pushIfNeeded();

		let reviewId: string | undefined;
		let prNumber: number | undefined;

		// Even if createButlerRequest is false, if we _cant_ create a PR, then
		// We want to always create the BR, and vice versa.
		if ((canPublishBR && $createButlerRequest) || !canPublishPR) {
			const reviewId = await publishBranch({
				projectId,
				stackId,
				topBranch: branch.name,
				user: $user
			});
			if (!reviewId) {
				posthog.capture('Butler Review Creation Failed');
				return;
			}
			posthog.capture('Butler Review Created');
			butRequestDetailsService.setDetails(reviewId, prTitle.value, prBody.value);
		}
		if ((canPublishPR && $createPullRequest) || !canPublishBR) {
			const pr = await createPr({
				title: prTitle.value,
				body: shouldAddPrBody() ? prBody.value : '',
				draft: $createDraft,
				upstreamBranchName
			});
			prNumber = pr?.number;
		}

		if (reviewId && prNumber && $project?.api?.repository_id) {
			brToPrService.refreshButRequestPrDescription(prNumber, reviewId, $project.api.repository_id);
		}

		prBody.reset();
		prTitle.reset();

		onClose();
	}

	async function createPr(params: CreatePrParams): Promise<PullRequest | undefined> {
		if (!forge) {
			error('Pull request service not available');
			return;
		}
		if (!branch) {
			return;
		}

		// All ids that existed prior to creating a new one (including archived).
		const prNumbers = branches.map((branch) => branch.prNumber);

		try {
			if (!baseBranchName) {
				error('No base branch name determined');
				return;
			}

			if (!params.upstreamBranchName) {
				error('No upstream branch name determined');
				return;
			}

			if (!prService) {
				error('Pull request service not available');
				return;
			}

			// Find the index of the current branch so we know where we want to point the pr.
			const currentIndex = branches.findIndex((b) => b.name === branch.name);
			if (currentIndex === -1) {
				throw new Error('Branch index not found.');
			}

			// Use base branch as base unless it's part of stack and should be be pointing
			// to the preceding branch. Ensuring we're not using `archived` branches as base.
			let base = baseBranch?.shortName || 'master';

			if (
				branchParent &&
				branchParent.prNumber &&
				branchParentDetails &&
				branchParentDetails.pushStatus !== 'integrated'
			) {
				base = branchParent.name;
			}

			const pr = await prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName: base,
				upstreamName: params.upstreamBranchName
			});

			// Store the new pull request number with the branch data.
			await updateBranchPrNumber({ projectId, stackId, branchName, prNumber: pr.number });

			// If we now have two or more pull requests we add a stack table to the description.
			prNumbers[currentIndex] = pr.number;
			const definedPrNumbers = prNumbers.filter(isDefined);
			if (definedPrNumbers.length > 0) {
				updatePrStackInfo(prService, definedPrNumbers);
			}
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError('Error while creating pull request', err);
		}
	}

	const isCreateButtonEnabled = $derived.by(() => {
		if ((canPublishBR && $createButlerRequest) || !canPublishPR) {
			return true;
		}
		if ((canPublishPR && $createPullRequest) || !canPublishBR) {
			return true;
		}
		return false;
	});

	async function onAiButtonClick() {
		if (!aiGenEnabled || aiIsLoading) return;

		aiIsLoading = true;
		await tick();

		let firstToken = true;

		try {
			const description = await aiService?.describePR({
				title: prTitle.value,
				body: prBody.value,
				commitMessages: commits.map((c) => c.message),
				prBodyTemplate: prBody.templateBody,
				onToken: (token) => {
					if (firstToken) {
						prBody.reset();
						firstToken = false;
					}
					prBody.append(token, true);
				}
			});

			if (description) {
				prBody.set(description, true);
			}
		} finally {
			aiIsLoading = false;
			await tick();
		}
	}

	export const imports = {
		get creationEnabled() {
			return isCreateButtonEnabled;
		},
		get isLoading() {
			return isExecuting;
		}
	};
</script>

<!-- HEADER -->

<!-- MAIN FIELDS -->
<div class="pr-content">
	<Textbox
		autofocus
		size="large"
		placeholder="PR title"
		bind:this={titleInput}
		value={prTitle.value}
		disabled={isExecuting}
		oninput={(value: string) => {
			prTitle.set(value);
		}}
		onkeydown={(e: KeyboardEvent) => {
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				prBody.descriptionInput?.focus();
			}
		}}
	/>

	<!-- PR TEMPLATE SELECT -->
	{#if templates.length > 0}
		<PrTemplateSection
			bind:selectedTemplate={prBody.templateBody}
			{templates}
			disabled={isExecuting}
		/>
	{/if}

	<!-- DESCRIPTION FIELD -->
	<MessageEditor
		bind:this={prBody.descriptionInput}
		{projectId}
		disabled={isExecuting}
		initialValue={prBody.value}
		enableFileUpload
		placeholder={'PR Description'}
		{onAiButtonClick}
		{canUseAI}
		{aiIsLoading}
		onChange={(text: string) => {
			prBody.set(text);
		}}
		onKeyDown={(e: KeyboardEvent) => {
			if (e.key === 'Tab' && e.shiftKey) {
				e.preventDefault();
				titleInput?.focus();
				return true;
			}

			if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
				e.preventDefault();
				createReview();
				return true;
			}

			return false;
		}}
	/>

	{#if canPublishBR && canPublishPR}
		<div class="options text-13">
			<label for="create-br" class="option-card">
				<div class="option-card-header" class:selected={$createButlerRequest}>
					<div class="option-card-header-content">
						<div class="option-card-header-title text-semibold">
							<Icon name="bowtie" />
							Create Butler Request
						</div>
						<span class="options__learn-more">
							<Link href="https://docs.gitbutler.com/review/overview">Learn more</Link>
						</span>
					</div>
					<div class="option-card-header-action">
						<Checkbox disabled={isExecuting} name="create-br" bind:checked={$createButlerRequest} />
					</div>
				</div>
			</label>

			<div class="option-card">
				<label
					for="create-pr"
					class="option-card-header has-settings"
					class:selected={$createPullRequest}
				>
					<div class="option-card-header-content">
						<div class="option-card-header-title text-semibold">
							<Icon name="github" />
							Create Pull Request
						</div>
					</div>

					<div class="option-card-header-action">
						<Checkbox name="create-pr" bind:checked={$createPullRequest} />
					</div>
				</label>
				<label
					for="create-pr-draft"
					class="option-subcard-drafty"
					class:disabled={!$createPullRequest}
				>
					<span class="text-semibold">PR Draft</span>
					<Toggle disabled={isExecuting} id="create-pr-draft" bind:checked={$createDraft} />
				</label>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.pr-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
		overflow: hidden;
	}

	.options {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
		align-items: stretch;
		width: 100%;
	}

	.option-card {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-m);
		overflow: hidden;
	}

	/* OPTION BOX */
	.option-card-header {
		display: flex;
		flex-grow: 1;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		padding: 12px;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&.has-settings {
			border-radius: var(--radius-m) var(--radius-m) 0 0;
		}

		&.selected {
			background-color: var(--clr-theme-pop-bg);
			border-color: var(--clr-theme-pop-element);
		}
	}

	.option-card-header-content {
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
		gap: 10px;
		flex-grow: 1;
	}

	.option-card-header-title {
		display: flex;
		gap: 8px;
		align-items: center;
	}

	.option-card-header-action {
		flex-grow: 0;

		display: block;
	}

	.option-subcard-drafty {
		padding: 12px;
		display: flex;
		justify-content: space-between;
		align-items: center;

		border-radius: 0 0 var(--radius-m) var(--radius-m);
		border: 1px solid var(--clr-border-2);
		border-top: none;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		&.disabled {
			pointer-events: none;
			cursor: not-allowed;
			opacity: 0.5;
			background-color: var(--clr-bg-2);
		}
	}

	.options__learn-more {
		color: var(--clr-text-2);
	}
</style>
