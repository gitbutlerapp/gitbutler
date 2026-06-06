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
	import MessageEditor from "$components/editor/MessageEditor.svelte";
	import MessageEditorInput from "$components/editor/MessageEditorInput.svelte";
	import PrTemplateSection from "$components/forge/PrTemplateSection.svelte";
	import { AI_SERVICE } from "$lib/ai/service";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { getBranchNameFromRef } from "$lib/branches/branchUtils";
	import { splitMessage } from "$lib/commits/commitMessage";
	import { projectAiGenEnabled, projectRunCommitHooks } from "$lib/config/config";
	import { showError } from "$lib/error/showError";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { mapErrorToToast } from "$lib/forge/github/errorMap";
	import { type PullRequest } from "$lib/forge/interface/types";
	import { PrPersistedStore } from "$lib/forge/prContents";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { updatePrDescriptionTables as updatePrStackInfo } from "$lib/forge/shared/prFooter";
	import { showToast } from "$lib/notifications/toasts";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { requiresPush } from "$lib/stacks/stack";
	import { type BranchPushResult } from "$lib/stacks/stackEndpoints";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { sleep } from "$lib/utils/sleep";
	import { inject } from "@gitbutler/core/context";
	import { persisted } from "@gitbutler/shared/persisted";
	import { chipToasts, TestId } from "@gitbutler/ui";
	import { IME_COMPOSITION_HANDLER } from "@gitbutler/ui/utils/imeHandling";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import { tick, untrack } from "svelte";
	import type { Commit, Segment } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		branchName: string;
		segment: Segment;
		branchIndex: number;
		parent: Segment | undefined;
		withForce: boolean;
		stackPrNumbers: (number | undefined)[];
		reviewId?: string;
		onClose: () => void;
	};

	const {
		projectId,
		stackId,
		branchName,
		segment,
		branchIndex,
		parent,
		withForce,
		stackPrNumbers,
		onClose,
	}: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchQuery.response);
	const baseBranchName = $derived(baseBranch?.shortName);
	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const reviewUnitAbbr = $derived(forgeInfo?.unit.abbr ?? "PR");
	const stackService = inject(STACK_SERVICE);
	const aiService = inject(AI_SERVICE);
	const uiState = inject(UI_STATE);
	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	const [pushStack, stackPush] = stackService.pushStack;

	const branchParentName = $derived(parent?.refName?.displayName);
	const branchParentPrNumber = $derived(parent?.metadata?.review.pullRequest);
	const branchParentDetails = $derived(parent);
	const branchDetails = $derived(segment);
	const commits = $derived(segment.commits);
	const runHooks = $derived(projectRunCommitHooks(projectId));

	// `forgeBranch` originally returned a Forge.branch handle; it was only
	// used as a truthy check that the forge knows about this branch. With the
	// new architecture we treat it as known whenever we have forge info.
	const forgeBranch = $derived(branchName ? !!forgeInfo : undefined);

	const createDraft = persisted<boolean>(false, "createDraftPr");

	const pushBeforeCreate = $derived(
		!forgeBranch || (branchDetails ? requiresPush(branchDetails.pushStatus) : true),
	);

	let titleInput = $state<HTMLTextAreaElement | undefined>(undefined);
	let messageEditor = $state<MessageEditor>();
	const imeHandler = inject(IME_COMPOSITION_HANDLER);

	// AI things
	const aiGenEnabled = projectAiGenEnabled(untrack(() => projectId));
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
	const isSubmittingReview = $derived(stackPush.current.isLoading || isCreatingReview);

	async function getDefaultTitle(commits: Commit[]): Promise<string> {
		const autoFill = $appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true;
		if (autoFill && commits.length === 1) {
			const commitMessage = commits[0]!.message;
			const { title } = splitMessage(commitMessage);
			return title;
		}
		return branchName;
	}

	const templatePath = persisted<string | undefined>(
		undefined,
		`last-template-${untrack(() => projectId)}`,
	);
	const templateEnabled = persisted(false, `enable-template-${untrack(() => projectId)}`);

	async function getDefaultBody(commits: Commit[]): Promise<string> {
		if ($templateEnabled && $templatePath) {
			return await stackService.template(projectId, forgeInfo?.name ?? "default", $templatePath);
		}
		const autoFill = $appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true;
		if (autoFill && commits.length === 1) {
			return splitMessage(commits[0]!.message).description;
		}
		return "";
	}

	const prTitle = $derived(
		new PrPersistedStore({
			cacheKey: "prtitle_" + projectId + "_" + branchName,
			commits,
			defaultFn: getDefaultTitle,
		}),
	);

	const prBody = $derived(
		new PrPersistedStore({
			cacheKey: "prbody" + projectId + "_" + branchName,
			commits,
			defaultFn: getDefaultBody,
		}),
	);

	$effect(() => {
		prBody.setDefault(commits);
		prTitle.setDefault(commits);
	});

	async function pushIfNeeded(
		branchName: string,
	): Promise<[string | undefined, BranchPushResult | undefined]> {
		if (!stackId) return [undefined, undefined];

		if (pushBeforeCreate) {
			const firstPush = branchDetails?.pushStatus === "completelyUnpushed";
			const pushQuery = await pushStack({
				projectId,
				stackId,
				withForce,
				skipForcePushProtection: false, // override available for regular push
				branch: branchName,
				runHooks: $runHooks,
				pushOpts: [],
			});

			if (firstPush) {
				// TODO: fix this hack for reactively available prService.
				await sleep(500);
			}

			const remoteRef = pushQuery.branchToRemote.find(([branch]) => branch === branchName)?.[1];

			const upstreamBranchName = remoteRef
				? getBranchNameFromRef(remoteRef, pushQuery.remote)
				: undefined;

			return [upstreamBranchName, pushQuery];
		}

		return [branchName, undefined];
	}

	export async function createReview() {
		if (isExecuting) return;

		const effectivePRBody = (await messageEditor?.getPlaintext()) ?? "";
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

			const [branch, pushQuery] = await pushIfNeeded(closureBranchName);

			await createPr({
				stackId: closureStackId,
				branchName: closureBranchName,
				title,
				body,
				draft,
				upstreamBranchName: branch,
			});

			prBody.reset();
			prTitle.reset();
			uiState.project(projectId).exclusiveAction.set(undefined);

			if (pushQuery) {
				const upstreamBranchNames = pushQuery.branchToRemote
					.map(([_, refname]) => getBranchNameFromRef(refname, pushQuery.remote))
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
		// Local mutable copy of pre-computed pr numbers so we can splice in
		// the newly-created pr number below.
		const prNumbers = [...stackPrNumbers];

		try {
			if (!baseBranchName) {
				chipToasts.error("No base branch name determined");
				return;
			}

			if (!params.upstreamBranchName) {
				chipToasts.error("No upstream branch name determined");
				return;
			}

			const currentIndex = branchIndex;
			if (currentIndex === -1) {
				throw new Error("Branch index not found.");
			}

			// Use base branch as base unless it's part of stack and should be be pointing
			// to the preceding branch. Ensuring we're not using `archived` branches as base.
			let base = baseBranch?.shortName || "master";

			if (
				branchParentName &&
				branchParentPrNumber &&
				branchParentDetails &&
				branchParentDetails.pushStatus !== "integrated"
			) {
				base = branchParentName;
			}

			const pr = await prService.createPr(projectId, {
				title: params.title,
				body: params.body,
				draft: params.draft,
				baseBranchName: base,
				upstreamName: params.upstreamBranchName,
				posthogLabel: forgeInfo?.posthogLabel,
			});

			// Store the new pull request number with the branch data.
			if (params.stackId) {
				await stackService.updateBranchPrNumber({
					projectId,
					stackId: params.stackId,
					branchName: params.branchName,
					prNumber: pr.number,
				});
			}

			// If we now have two or more pull requests we add a stack table to the description.
			prNumbers[currentIndex] = pr.number;
			const definedPrNumbers = prNumbers.filter(isDefined);
			if (definedPrNumbers.length > 0) {
				updatePrStackInfo(prService, projectId, definedPrNumbers, forgeInfo?.unit.symbol);
			}

			// Show success notification
			const unit = forgeInfo?.unit.abbr || "PR";
			const symbol = forgeInfo?.unit.symbol || "#";
			chipToasts.success(`${unit} ${symbol}${pr.number} created successfully`);

			return pr;
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError("Error while creating pull request", err);
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
				},
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
			return isSubmittingReview;
		},
		get isExecuting() {
			return isExecuting;
		},
	};
</script>

<div class="pr-editor">
	<PrTemplateSection
		{projectId}
		template={{ enabled: templateEnabled, path: templatePath }}
		forgeName={forgeInfo?.name ?? "default"}
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
			onkeydown={imeHandler.handleKeydown((e: KeyboardEvent) => {
				if (e.key === "Enter" || (e.key === "Tab" && !e.shiftKey)) {
					e.preventDefault();
					messageEditor?.focus();
				}

				if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
					e.preventDefault();
					createReview();
					return true;
				}

				if (e.key === "Escape") {
					e.preventDefault();
					onClose();
				}
			})}
			placeholder="{reviewUnitAbbr} title"
			showCount={false}
			oninput={imeHandler.handleInput((e: Event) => {
				const target = e.target as HTMLInputElement;
				prTitle.set(target.value);
			})}
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
			placeholder="{reviewUnitAbbr} Description"
			messageType="pr"
			{reviewUnitAbbr}
			{onAiButtonClick}
			{canUseAI}
			{aiIsLoading}
			onChange={(text: string) => {
				prBody.set(text);
			}}
			onKeyDown={(e: KeyboardEvent) => {
				if (e.key === "Tab" && e.shiftKey) {
					e.preventDefault();
					titleInput?.focus();
					return true;
				}

				if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
					e.preventDefault();
					createReview();
					return true;
				}

				if (e.key === "Escape") {
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
