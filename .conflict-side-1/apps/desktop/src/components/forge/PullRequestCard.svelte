<script lang="ts">
	import PrStatusBadge from "$components/forge/PrStatusBadge.svelte";
	import PrStatusPoller from "$components/forge/PrStatusPoller.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { CHECKS_MONITOR } from "$lib/forge/checksMonitor.svelte";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { REPO_SERVICE } from "$lib/forge/repoService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		Button,
		ContextMenu,
		ContextMenuItem,
		ContextMenuSection,
		Icon,
		AvatarGroup,
		TestId,
	} from "@gitbutler/ui";
	import { getForgeLogo } from "@gitbutler/ui/utils/getForgeLogo";
	import type { PullRequest } from "$lib/forge/interface/types";
	import type { Snippet } from "svelte";

	type ButtonStatus = {
		disabled: boolean;
		tooltip?: string;
	};

	interface Props {
		projectId: string;
		testId?: string;
		branchName: string;
		poll?: boolean;
		prNumber: number;
		isPushed?: boolean;
		hasParent?: boolean;
		baseIsTargetBranch?: boolean;
		parentIsPushed?: boolean;
		button?: Snippet<
			[
				{
					pr: PullRequest;
					mergeStatus: ButtonStatus;
					reopenStatus: ButtonStatus;
					setDraft: (draft: boolean) => Promise<void>;
				},
			]
		>;
	}

	const {
		projectId,
		testId,
		poll,
		prNumber,
		isPushed,
		hasParent,
		baseIsTargetBranch,
		parentIsPushed,
		button,
	}: Props = $props();

	let contextMenuOpen = $state(false);
	let contextMenuTarget = $state<MouseEvent | HTMLElement>();
	let container = $state<HTMLElement>();
	let hasChecks = $state(false);

	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const prService = inject(PR_SERVICE);
	const repoService = inject(REPO_SERVICE);
	const checksMonitor = inject(CHECKS_MONITOR);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const forgeName = $derived(forgeInfo?.name ?? "default");
	const repoInfoEnabled = $derived(!!forgeInfo?.capabilities.repoInfo);
	const checksEnabled = $derived(!!forgeInfo?.capabilities.checks);

	const prQuery = $derived(prService.get(projectId, prNumber, { forceRefetch: true }));
	const pr = $derived(prQuery.response);
	const mergeStatusQuery = $derived(prService.getMergeStatus(projectId, prNumber));
	const prMergeStatus = $derived(mergeStatusQuery.response);
	const repoQuery = $derived(repoInfoEnabled ? repoService.getInfo(projectId) : undefined);
	const repoInfo = $derived(repoQuery?.response);

	const name = $derived(forgeInfo?.unit.name ?? "Pull request");
	const abbr = $derived(forgeInfo?.unit.abbr ?? "PR");
	const symbol = $derived(forgeInfo?.unit.symbol ?? "#");

	let draftToggling = $state(false);

	async function handleSetDraft(draft: boolean) {
		if (draftToggling) return;
		draftToggling = true;
		try {
			await prService.setDraft(projectId, prNumber, draft);
			await prService.fetch(projectId, prNumber, { forceRefetch: true });
		} finally {
			draftToggling = false;
		}
	}

	const mergeStatus = $derived.by(() => {
		let disabled = true;
		let tooltip = undefined;
		if (isPushed && hasParent && !parentIsPushed) {
			tooltip = "Remote parent branch seems to have been deleted";
		} else if (!baseIsTargetBranch) {
			tooltip = name + " is not next in stack";
		} else if (repoInfoEnabled && !repoInfo?.canMerge) {
			// Forges without a repoService (e.g. Bitbucket, Azure) rely
			// on the server-side merge button to refuse.
			tooltip = name + " requires push permissions";
		} else if (pr?.draft) {
			tooltip = name + " is a draft";
		} else if (prMergeStatus?.mergeableState === "blocked") {
			tooltip = name + " needs approval";
		} else if (prMergeStatus?.mergeableState === "unknown") {
			tooltip = name + " mergeability is unknown";
		} else if (prMergeStatus?.mergeableState === "behind") {
			tooltip = name + " base is too far behind";
		} else if (prMergeStatus?.mergeableState === "dirty") {
			tooltip = name + " has conflicts";
		} else if (!prMergeStatus?.isMergeable) {
			tooltip = name + " is not mergeable";
		} else {
			disabled = false;
		}
		return { disabled, tooltip };
	});

	const reopenStatus = $derived.by(() => {
		let disabled = true;
		let tooltip = undefined;
		if (isPushed && hasParent && !parentIsPushed) {
			tooltip = "Remote parent branch seems to have been deleted";
		} else {
			disabled = false;
		}
		return { disabled, tooltip };
	});
</script>

<ReduxResult result={prQuery?.result} projectId="dummy">
	{#snippet children(pr)}
		{#if poll}
			<PrStatusPoller {projectId} number={pr.number} />
		{/if}

		{#if contextMenuOpen}
			<ContextMenu
				target={contextMenuTarget}
				onclose={() => {
					contextMenuOpen = false;
				}}
			>
				<ContextMenuSection>
					<ContextMenuItem
						label="Open in browser"
						onclick={() => {
							contextMenuOpen = false;
							urlService.openExternalUrl(pr.htmlUrl);
						}}
					/>
					<ContextMenuItem
						label="Copy link"
						onclick={() => {
							contextMenuOpen = false;
							clipboardService.write(pr.htmlUrl, { message: `${abbr} link copied` });
						}}
					/>
					<ContextMenuItem
						label="Refetch status"
						onclick={() => {
							contextMenuOpen = false;
							prService.fetch(projectId, pr.number, { forceRefetch: true });
							if (hasChecks && checksEnabled) {
								checksMonitor.fetch(projectId, pr.sourceBranch, { forceRefetch: true });
							}
						}}
					/>
					{#if !pr.closedAt && !pr.mergedAt}
						<ContextMenuItem
							label={pr.draft ? "Ready for review" : "Convert to draft"}
							disabled={draftToggling}
							onclick={async () => {
								contextMenuOpen = false;
								await handleSetDraft(!pr.draft);
							}}
						/>
					{/if}
				</ContextMenuSection>
				{#if hasChecks}
					<ContextMenuSection>
						<ContextMenuItem
							label="Open checks"
							onclick={() => {
								contextMenuOpen = false;
								urlService.openExternalUrl(`${pr.htmlUrl}/checks`);
							}}
						/>
						<ContextMenuItem
							label="Copy checks"
							onclick={() => {
								contextMenuOpen = false;
								clipboardService.write(`${pr.htmlUrl}/checks`, { message: "Checks link copied" });
							}}
						/>
					</ContextMenuSection>
				{/if}
			</ContextMenu>
		{/if}

		<div
			data-testid={testId}
			bind:this={container}
			role="article"
			class="review-card pr-card"
			oncontextmenu={(e: MouseEvent) => {
				e.preventDefault();
				e.stopPropagation();
				contextMenuTarget = e;
				contextMenuOpen = true;
			}}
		>
			<div class="pr-actions">
				<Button
					kind="outline"
					size="tag"
					icon="copy"
					tooltip="Copy {abbr} link"
					onclick={() => {
						clipboardService.write(pr.htmlUrl, { message: `${abbr} link copied` });
					}}
				/>
				<Button
					kind="outline"
					size="tag"
					icon="arrow-up-righ"
					tooltip="Open {abbr} in browser"
					onclick={() => {
						urlService.openExternalUrl(pr.htmlUrl);
					}}
				/>
			</div>

			<div class="text-13 text-semibold pr-row">
				<Icon name={getForgeLogo(forgeName)} />
				<h4 class="text-14 text-semibold">
					{`${abbr} ${symbol}${pr.number}`}
				</h4>

				<PrStatusBadge testId={TestId.PRStatusBadge} {pr} />
			</div>
			<div class="text-12 pr-row">
				<div class="factoid">
					{#if pr.reviewers.length > 0}
						<span class="label">Reviewers:</span>
						<div class="avatar-group-container">
							<AvatarGroup
								avatars={pr.reviewers.map((r) => ({
									srcUrl: r.srcUrl,
									username: r.name,
								}))}
							/>
						</div>
					{:else}
						<span class="label italic">No reviewers</span>
					{/if}
				</div>
				<span class="separator">•</span>
				<div class="factoid">
					<span class="label">
						<Icon name="chat" size={14} />
					</span>
					<span>{prMergeStatus?.commentsCount ?? 0}</span>
				</div>
			</div>

			{#if button}
				<div class="pr-row">
					{@render button({ pr, mergeStatus, reopenStatus, setDraft: handleSetDraft })}
				</div>
			{/if}
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.pr-row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;

		&:empty {
			display: none;
		}
	}

	.factoid {
		display: flex;
		align-items: center;
		gap: 4px;

		> .label {
			display: flex;
			color: var(--text-2);

			&.italic {
				font-style: italic;
			}
		}
	}

	.separator {
		transform: translateY(-1.5px);
		color: var(--text-3);
	}

	.pr-actions {
		display: flex;
		position: absolute;
		top: 8px;
		right: 8px;
		gap: 4px;
	}
</style>
