<script lang="ts">
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { descriptionTitle } from "$lib/commits/commit";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { showToast } from "$lib/notifications/toasts";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import {
		buildWorkspaceUpstreamRows,
		getWorkspaceBottomUpdates,
		listIntegratedBranchesToDelete,
		type WorkspaceUpstreamAction,
		type WorkspaceUpstreamRow,
		type WorkspaceUpstreamSelection,
	} from "$lib/upstream/workspaceUpstreamIntegration";
	import { WORKSPACE_UPSTREAM_INTEGRATION_SERVICE } from "$lib/upstream/workspaceUpstreamIntegrationService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		getBooleanStorageItem,
		removeStorageItem,
		setBooleanStorageItem,
	} from "@gitbutler/shared/persisted";
	import {
		AsyncButton,
		Badge,
		Button,
		IntegrationSeriesRow,
		Modal,
		ScrollableContainer,
		Select,
		SelectItem,
		SimpleCommitRow,
		TestId,
		type BranchShouldBeDeletedMap,
	} from "@gitbutler/ui";
	import { tick } from "svelte";
	import { SvelteMap } from "svelte/reactivity";
	import type { RefInfo } from "@gitbutler/but-sdk";

	type OperationState = "inert" | "loading" | "completed";

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const emptyHeadInfo: RefInfo = {
		workspaceRef: null,
		stacks: [],
		target: null,
		isManagedRef: true,
		isManagedCommit: true,
		isEntrypoint: true,
	};

	const workspaceUpstreamIntegrationService = inject(WORKSPACE_UPSTREAM_INTEGRATION_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchQuery.response);
	const currentHeadInfoQuery = $derived(workspaceUpstreamIntegrationService.headInfo(projectId));
	const currentHeadInfo = $derived(currentHeadInfoQuery.response ?? emptyHeadInfo);
	const urlService = inject(URL_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>("inert");
	let previewLoading = $state(false);
	let previewHeadInfo = $state<RefInfo | undefined>();
	let previewError = $state<string | undefined>();
	let previewRequestId = 0;
	const selections = new SvelteMap<string, WorkspaceUpstreamSelection>();
	const rows = $derived(buildWorkspaceUpstreamRows(currentHeadInfo, previewHeadInfo));
	const [integrateUpstream] = $derived(workspaceUpstreamIntegrationService.integrateUpstream());

	function getDontDeleteBranchStorageKey(branchName: string): string {
		return `integrate-upstream-modal:dont-delete-branch:${projectId}:${branchName}`;
	}

	function defaultDeleteIntegratedBranches(branchNames: string[]): boolean {
		return !branchNames.some((branchName) =>
			getBooleanStorageItem(getDontDeleteBranchStorageKey(branchName)),
		);
	}

	function ensureSelections() {
		for (const row of rows) {
			if (selections.has(row.stackKey)) continue;
			selections.set(row.stackKey, {
				stackKey: row.stackKey,
				action: "rebase",
				deleteIntegratedBranches: defaultDeleteIntegratedBranches(row.branchNames),
			});
		}
	}

	$effect(() => {
		if (!modal?.imports.open) return;
		ensureSelections();
	});

	$effect(() => {
		if (!modal?.imports.open) return;
		const requestId = ++previewRequestId;
		previewLoading = true;
		previewError = undefined;
		workspaceUpstreamIntegrationService
			.preview({
				projectId,
				updates: getWorkspaceBottomUpdates(currentHeadInfo, selections),
			})
			.then((workspace) => {
				if (requestId !== previewRequestId) return;
				previewHeadInfo = workspace.headInfo;
			})
			.catch((error: unknown) => {
				console.log(error);
				if (requestId !== previewRequestId) return;
				previewHeadInfo = undefined;
				previewError = error instanceof Error ? error.message : String(error);
			})
			.finally(() => {
				if (requestId !== previewRequestId) return;
				previewLoading = false;
			});
	});

	function integrationOptions(
		row: WorkspaceUpstreamRow,
	): { label: string; value: WorkspaceUpstreamAction }[] {
		return row.canMerge
			? [
					{ label: "Rebase", value: "rebase" },
					{ label: "Merge", value: "merge" },
				]
			: [{ label: "Rebase", value: "rebase" }];
	}

	function getBranchShouldBeDeletedMap(row: WorkspaceUpstreamRow): BranchShouldBeDeletedMap {
		return Object.fromEntries(
			row.branchNames.map((branchName) => [
				branchName,
				selections.get(row.stackKey)?.deleteIntegratedBranches ?? false,
			]),
		) as BranchShouldBeDeletedMap;
	}

	function updateBranchShouldBeDeletedMap(
		row: WorkspaceUpstreamRow,
		shouldBeDeleted: boolean,
	): void {
		const selection = selections.get(row.stackKey);
		if (!selection) return;

		for (const branchName of row.branchNames) {
			const key = getDontDeleteBranchStorageKey(branchName);
			if (!shouldBeDeleted) {
				setBooleanStorageItem(key, true);
			} else {
				removeStorageItem(key);
			}
		}

		selections.set(row.stackKey, { ...selection, deleteIntegratedBranches: shouldBeDeleted });
	}

	async function integrate() {
		if (previewLoading || previewError || !previewHeadInfo) return;

		integratingUpstream = "loading";
		await tick();

		const updates = getWorkspaceBottomUpdates(currentHeadInfo, selections);
		const integrationResult =
			updates.length > 0
				? await integrateUpstream({
						projectId,
						updates,
					})
				: undefined;

		const cleanupRows = buildWorkspaceUpstreamRows(
			currentHeadInfo,
			integrationResult?.headInfo ?? currentHeadInfo,
		);
		const fullyIntegratedRows = cleanupRows.filter((row) => row.isFullyIntegrated && row.stackId);
		for (const row of fullyIntegratedRows) {
			await stackService.unapply({ projectId, stackId: row.stackId! });
		}

		const branchesToDelete = listIntegratedBranchesToDelete(cleanupRows, selections);
		for (const branchName of branchesToDelete) {
			await stackService
				.deleteLocalBranch({
					projectId,
					refname: `refs/heads/${branchName}`,
					givenName: branchName,
				})
				.catch((error: unknown) => {
					showToast({
						title: "Local branch cleanup failed",
						message: error instanceof Error ? error.message : String(error),
						style: "warning",
					});
				});
		}

		await baseBranchService.refreshBaseBranch(projectId);
		integratingUpstream = "completed";
		modal?.close();
	}

	export async function show() {
		integratingUpstream = "inert";
		previewHeadInfo = undefined;
		previewError = undefined;
		selections.clear();
		await workspaceUpstreamIntegrationService.fetchHeadInfo(projectId);
		ensureSelections();
		await tick();
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		},
	};
</script>

{#snippet stackStatus(row: WorkspaceUpstreamRow)}
	<IntegrationSeriesRow
		testId={TestId.IntegrateUpstreamSeriesRow}
		series={row.series}
		branchShouldBeDeletedMap={getBranchShouldBeDeletedMap(row)}
		updateBranchShouldBeDeletedMap={(_branchNames, shouldBeDeleted) =>
			updateBranchShouldBeDeletedMap(row, shouldBeDeleted)}
	>
		{#if !row.isFullyIntegrated}
			<Select
				value={selections.get(row.stackKey)?.action ?? "rebase"}
				maxWidth={130}
				onselect={(value) => {
					const selection = selections.get(row.stackKey) ?? {
						stackKey: row.stackKey,
						action: "rebase",
						deleteIntegratedBranches: defaultDeleteIntegratedBranches(row.branchNames),
					};
					selections.set(row.stackKey, { ...selection, action: value });
				}}
				options={integrationOptions(row)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</IntegrationSeriesRow>
{/snippet}

<Modal
	testId={TestId.IntegrateUpstreamCommitsModal}
	bind:this={modal}
	{onClose}
	width={520}
	noPadding
	onSubmit={() => {
		if (previewLoading || previewError || !previewHeadInfo) return;
		return integrate();
	}}
>
	<ScrollableContainer maxHeight="70vh">
		{#if base}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming {base.upstreamCommits.length === 1 ? "change" : "changes"}</span><Badge
						>{base.upstreamCommits.length}</Badge
					>
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="16.5rem">
						{#each base.upstreamCommits as commit}
							{@const commitUrl = forge.current.commitUrl(commit.id)}
							<SimpleCommitRow
								title={descriptionTitle(commit) ?? ""}
								sha={commit.id}
								date={new Date(commit.createdAt)}
								author={commit.author.name}
								url={commitUrl}
								onOpen={(url) => urlService.openExternalUrl(url)}
								onCopy={() => clipboardService.write(commit.id, { message: "Commit hash copied" })}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}

		{#if previewError}
			<div class="section">
				<h3 class="text-14 text-semibold">Preview unavailable</h3>
				<p class="text-12 clr-text-2">{previewError}</p>
			</div>
		{/if}

		{#if rows.length > 0}
			<div class="section">
				<h3 class="text-14 text-semibold">To be updated:</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="15rem">
						{#each rows as row}
							{@render stackStatus(row)}
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
	</ScrollableContainer>

	{#snippet controls()}
		<div class="controls">
			<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
			<AsyncButton
				testId={TestId.IntegrateUpstreamActionButton}
				wide
				style="pop"
				disabled={previewLoading || !!previewError || !previewHeadInfo}
				loading={integratingUpstream === "loading" || previewLoading}
				action={async () => {
					await integrate();
				}}
			>
				Update workspace
			</AsyncButton>
		</div>
	{/snippet}
</Modal>

<style>
	.section {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 14px;
		border-bottom: 1px solid var(--border-2);

		&:last-child {
			border-bottom: none;
		}

		.scroll-wrap {
			overflow: hidden;
			border: 1px solid var(--border-2);
			border-radius: var(--radius-m);
		}
	}

	.section-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}
</style>
