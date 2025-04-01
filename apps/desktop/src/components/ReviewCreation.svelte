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
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { ButRequestDetailsService } from '$lib/forge/butRequestDetailsService';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { mapErrorToToast } from '$lib/forge/github/errorMap';
	import { type PullRequest } from '$lib/forge/interface/types';
	import { ReactivePRBody, ReactivePRTitle } from '$lib/forge/prContents.svelte';
	import {
		BrToPrService,
		updatePrDescriptionTables as updatePrStackInfo
	} from '$lib/forge/shared/prFooter';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { ProjectsService } from '$lib/project/projectsService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UserService } from '$lib/user/userService';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { sleep } from '$lib/utils/sleep';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

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

	const user = userService.user;
	const project = projectsService.getProjectStore(projectId);

	const [publishBranch] = stackService.publishBranch;
	const [updateBranchPrNumber] = stackService.updateBranchPrNumber;
	const [pushStack] = stackService.pushStack();

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branch = $derived(branchResult.current.data);
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branches = $derived(
		branchesResult.current.data?.filter((branch) => !branch.archived) || []
	);
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

	let templateBody = $state<string | undefined>(undefined);
	const pushBeforeCreate = $derived(
		!forgeBranch || commits.some((c) => c.state.type === 'LocalOnly')
	);

	// Displays template select component when true.
	let useTemplate = persisted(false, `use-template-${projectId}`);
	// Available pull request templates.
	let templates = $state<string[]>([]);

	const canPublishBR = $derived(!!($canPublish && branch?.name && !branch.reviewId));
	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));

	const prTitle = $derived(new ReactivePRTitle(projectId, undefined, commits, branch?.name ?? ''));

	const prBody = $derived(
		new ReactivePRBody(
			projectId,
			branch?.description ?? '',
			undefined,
			commits,
			templateBody,
			branch?.name ?? ''
		)
	);

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

	export async function createReview(close: () => void) {
		if (!branch) return;
		if (!$user) return;

		const upstreamBranchName = await pushIfNeeded();

		let reviewId: string | undefined;
		let prNumber: number | undefined;

		// Even if createButlerRequest is false, if we _cant_ create a PR, then
		// We want to always create the BR, and vice versa.
		if ((canPublishBR && $createButlerRequest) || !canPublishPR) {
			reviewId = await publishBranch({ projectId, stackId, topBranch: branch.name, user: $user });
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

		close();
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
				branchParentDetails &&
				branchParentDetails.pushStatus !== 'integrated' &&
				!branchParent?.archived
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
</script>

<!-- HEADER -->

<!-- MAIN FIELDS -->
<div class="pr-content">
	<div class="pr-fields">
		<Textbox
			placeholder="PR title"
			value={prTitle.value}
			oninput={(value: string) => {
				prTitle.set(value);
			}}
		/>

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
		<MessageEditor
			{projectId}
			{stackId}
			initialValue={prBody.value}
			onChange={(text: string) => {
				prBody.set(text);
			}}
		/>
	</div>
</div>
<div class="combined-controls">
	<div class="options">
		{#if canPublishBR}
			<div class="stacked-options">
				{#if canPublishPR}
					<div class="option">
						<p class="text-13">Create Butler Review</p>
						<Toggle bind:checked={$createButlerRequest} />
					</div>
					<div class="option text-13">
						<Link href="https://docs.gitbutler.com/review/overview">Learn more</Link>
					</div>
				{:else}
					<div class="option text-13">
						Creates a Butler Review <Link href="https://docs.gitbutler.com/review/overview"
							>Learn more</Link
						>
					</div>
				{/if}
			</div>
		{/if}
		{#if canPublishPR}
			<div class="stacked-options">
				{#if canPublishBR}
					<div class="option">
						<p class="text-13">Create Pull Request</p>
						<Toggle bind:checked={$createPullRequest} />
					</div>
				{/if}

				{#if $createPullRequest}
					<div class="option">
						<p class="text-13">Pull Request Kind:</p>
						<Select
							options={[
								{ label: 'Draft PR', value: 'draft' },
								{ label: 'PR', value: 'regular' }
							]}
							value={$createDraft ? 'draft' : 'regular'}
							autoWidth
							onselect={(value) => {
								$createDraft = value === 'draft';
							}}
						>
							{#snippet customSelectButton()}
								<Button kind="outline" icon="select-chevron" size="tag">
									{$createDraft ? 'Draft PR' : 'PR'}
								</Button>
							{/snippet}
							{#snippet itemSnippet({ item, highlighted })}
								<SelectItem {highlighted}>{item.label}</SelectItem>
							{/snippet}
						</Select>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.pr-content {
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	/* FIELDS */

	.pr-fields {
		display: flex;
		flex-direction: column;
		gap: 14px;
		min-height: 0;
	}

	/* PREVIEW */
	.combined-controls {
		display: flex;
		flex-direction: column;
		gap: 12px;
		width: 100%;
		padding-top: 16px;
	}

	.options {
		width: 100%;
		display: flex;
		gap: 12px;
		align-items: flex-start;
		justify-content: space-around;
	}

	.stacked-options {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.option {
		display: flex;
		gap: 12px;
		align-items: center;
		justify-content: space-between;
	}
</style>
