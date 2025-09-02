<script lang="ts" module>
	export interface CreatePrParams {
		stackId?: string;
		branchName: string;
		title: string;
		body: string;
		draft: boolean;
		upstreamBranchName: string | undefined;
	}
</script>

<script lang="ts">
	import PrTemplateSection from '$components/PrTemplateSection.svelte';
	import MessageEditor from '$components/editor/MessageEditor.svelte';
	import MessageEditorInput from '$components/editor/MessageEditorInput.svelte';
	import { AI_SERVICE } from '$lib/ai/service';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { type Commit } from '$lib/branches/v3';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { mapErrorToToast } from '$lib/forge/github/errorMap';
	import { GitHubPrService } from '$lib/forge/github/githubPrService.svelte';
	import { type PullRequest } from '$lib/forge/interface/types';
	import { PrPersistedStore } from '$lib/forge/prContents';
	import { updatePrDescriptionTables as updatePrStackInfo } from '$lib/forge/shared/prFooter';
	import { showError, showToast } from '$lib/notifications/toasts';
	import { REMOTES_SERVICE } from '$lib/remotes/remotesService';
	import { partialStackRequestsForcePush, requiresPush } from '$lib/stacks/stack';
	import { STACK_SERVICE, type BranchPushResult } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { parseRemoteUrl } from '$lib/url/gitUrl';
	import { USER_SERVICE } from '$lib/user/userService';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { splitMessage } from '$lib/utils/commitMessage';
	import { sleep } from '$lib/utils/sleep';
	import { inject } from '@gitbutler/core/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import { chipToasts, TestId } from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		reviewId?: string;
		onClose: () => void;
	};

	const { projectId, stackId, branchName, onClose }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchResponse.current.data);
	const baseBranchName = $derived(baseBranch?.shortName);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const prService = $derived(forge.current.prService);
	const stackService = inject(STACK_SERVICE);
	const userService = inject(USER_SERVICE);
	const aiService = inject(AI_SERVICE);
	const remotesService = inject(REMOTES_SERVICE);
	const uiState = inject(UI_STATE);

	const user = userService.user;

	const [pushStack, stackPush] = stackService.pushStack;

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
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchDetails = $derived(branchDetailsResult.current.data);
	const commitsResult = $derived(stackService.commits(projectId, stackId, branchName));
	const commits = $derived(commitsResult.current.data || []);

	const forgeBranch = $derived(branchName ? forge.current.branch(branchName) : undefined);

	const createDraft = persisted<boolean>(false, 'createDraftPr');

	const pushBeforeCreate = $derived(
		!forgeBranch || (branchDetails ? requiresPush(branchDetails.pushStatus) : true)
	);

	let titleInput = $state<HTMLTextAreaElement | undefined>(undefined);
	let messageEditor = $state<MessageEditor>();

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

	let isCreatingReview = $state<boolean>(false);
	const isExecuting = $derived(stackPush.current.isLoading || aiIsLoading || isCreatingReview);

	async function getDefaultTitle(commits: Commit[]): Promise<string> {
		if (commits.length === 1) {
			const commitMessage = commits[0]!.message;
			const { title } = splitMessage(commitMessage);
			return title;
		}
		return branchName;
	}

	const templatePath = persisted<string | undefined>(undefined, `last-template-${projectId}`);
	const templateEnabled = persisted(false, `enable-template-${projectId}`);

	async function getDefaultBody(commits: Commit[]): Promise<string> {
		if ($templateEnabled && $templatePath) {
			return await stackService.template(projectId, forge.current.name, $templatePath);
		}
		if (commits.length === 1) {
			return splitMessage(commits[0]!.message).description;
		}
		return '';
	}

	const prTitle = $derived(
		new PrPersistedStore({
			cacheKey: 'prtitle_' + projectId + '_' + branchName,
			commits,
			defaultFn: getDefaultTitle
		})
	);

	const prBody = $derived(
		new PrPersistedStore({
			cacheKey: 'prbody' + projectId + '_' + branchName,
			commits,
			defaultFn: getDefaultBody
		})
	);

	$effect(() => {
		prBody.setDefault(commits);
		prTitle.setDefault(commits);
	});

	async function pushIfNeeded(
		branchName: string
	): Promise<[string | undefined, BranchPushResult | undefined]> {
		if (pushBeforeCreate) {
			const firstPush = branchDetails?.pushStatus === 'completelyUnpushed';
			const withForce = partialStackRequestsForcePush(branchName, branches);
			const pushResult = await pushStack({
				projectId,
				stackId,
				withForce,
				skipForcePushProtection: false, // override available for regular push
				branch: branchName
			});

			if (firstPush) {
				// TODO: fix this hack for reactively available prService.
				await sleep(500);
			}

			const remoteRef = pushResult.branchToRemote.find(([branch]) => branch === branchName)?.[1];

			const upstreamBranchName = remoteRef
				? getBranchNameFromRef(remoteRef, pushResult.remote)
				: undefined;

			return [upstreamBranchName, pushResult];
		}

		return [branchName, undefined];
	}

	export async function createReview() {
		if (isExecuting) return;
		if (!$user) return;

		const effectivePRBody = (await messageEditor?.getPlaintext()) ?? '';
		// Declare early to have them inside the function closure, in case
		// the component unmounts or updates.
		const closureStackId = stackId;
		const closureBranchName = branchName;
		const title = $prTitle;
		const body = effectivePRBody;
		const draft = $createDraft;

		try {
			isCreatingReview = true;
			await tick();

			const [branch, pushResult] = await pushIfNeeded(closureBranchName);

			await createPr({
				stackId: closureStackId,
				branchName: closureBranchName,
				title,
				body,
				draft,
				upstreamBranchName: branch
			});

			prBody.reset();
			prTitle.reset();
			uiState.project(projectId).exclusiveAction.set(undefined);

			if (pushResult) {
				const upstreamBranchNames = pushResult.branchToRemote
					.map(([_, refname]) => getBranchNameFromRef(refname, pushResult.remote))
					.filter(isDefined);
				if (upstreamBranchNames.length === 0) return;
				uiState.project(projectId).branchesToPoll.add(...upstreamBranchNames);
			}
		} finally {
			isCreatingReview = false;
		}
		onClose();
	}

	async function createPr(params: CreatePrParams): Promise<PullRequest | undefined> {
		if (!forge) {
			chipToasts.error('Pull request service not available');
			return;
		}

		// All ids that existed prior to creating a new one (including archived).
		const prNumbers = branches.map((branch) => branch.prNumber);

		try {
			if (!baseBranchName) {
				chipToasts.error('No base branch name determined');
				return;
			}

			if (!params.upstreamBranchName) {
				chipToasts.error('No upstream branch name determined');
				return;
			}

			if (!prService) {
				chipToasts.error('Pull request service not available');
				return;
			}

			// Find the index of the current branch so we know where we want to point the pr.
			const currentIndex = branches.findIndex((b) => b.name === params.branchName);
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

			const pushRemoteName = baseBranch?.actualPushRemoteName();
			if (!pushRemoteName) {
				chipToasts.error('No push remote name determined');
				return;
			}

			const allRemotes = await remotesService.remotes(projectId);
			const pushRemote = allRemotes.find((r) => r.name === pushRemoteName);
			const pushRemoteUrl = pushRemote?.url;

			const repoInfo = parseRemoteUrl(pushRemoteUrl);

			const upstreamName =
				prService instanceof GitHubPrService
					? repoInfo?.owner
						? `${repoInfo.owner}:${params.upstreamBranchName}`
						: params.upstreamBranchName
					: params.upstreamBranchName;

			const pr = await prService.createPr({
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName: base,
				upstreamName
			});

			// Store the new pull request number with the branch data.
			if (params.stackId) {
				await stackService.updateBranchPrNumber({
					projectId,
					stackId: params.stackId,
					branchName: params.branchName,
					prNumber: pr.number
				});
			}

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

	async function onAiButtonClick() {
		if (!aiGenEnabled || aiIsLoading) return;

		aiIsLoading = true;
		await tick();

		let firstToken = true;

		try {
			const description = await aiService?.describePR({
				title: $prTitle,
				body: $prBody,
				commitMessages: commits.map((c) => c.message),
				prBodyTemplate: prBody.default,
				onToken: (token) => {
					if (firstToken) {
						prBody.reset();
						firstToken = false;
					}
					prBody.append(token);
					messageEditor?.setText($prBody);
				}
			});

			if (description) {
				prBody.set(description);
				messageEditor?.setText($prBody);
			}
		} finally {
			aiIsLoading = false;
			await tick();
		}
	}

	export const imports = {
		get isLoading() {
			return isExecuting;
		}
	};
</script>

<div class="pr-editor">
	<PrTemplateSection
		{projectId}
		template={{ enabled: templateEnabled, path: templatePath }}
		forgeName={forge.current.name}
		disabled={isExecuting}
		onselect={(value) => {
			prBody.set(value);
			messageEditor?.setText(value);
		}}
	/>
	<div class="pr-fields">
		<MessageEditorInput
			testId={TestId.ReviewTitleInput}
			bind:ref={titleInput}
			value={$prTitle}
			onchange={(value) => {
				prTitle.set(value);
			}}
			onkeydown={(e: KeyboardEvent) => {
				if (e.key === 'Enter' || (e.key === 'Tab' && !e.shiftKey)) {
					e.preventDefault();
					messageEditor?.focus();
				}

				if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
					e.preventDefault();
					createReview();
					return true;
				}

				if (e.key === 'Escape') {
					e.preventDefault();
					onClose();
				}
			}}
			placeholder="PR title"
			showCount={false}
			oninput={(e: Event) => {
				const target = e.target as HTMLInputElement;
				prTitle.set(target.value);
			}}
		/>
		<MessageEditor
			forceSansFont
			bind:this={messageEditor}
			testId={TestId.ReviewDescriptionInput}
			{projectId}
			disabled={isExecuting}
			initialValue={$prBody}
			enableFileUpload
			enableSmiles
			placeholder="PR Description"
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

				if (e.key === 'Escape') {
					e.preventDefault();
					onClose();
					return true;
				}

				return false;
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.pr-editor {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		gap: 10px;
	}

	.pr-fields {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}
</style>
