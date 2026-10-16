<script lang="ts">
	import BranchHeaderIcon from "$components/branch/BranchHeaderIcon.svelte";
	import { Avatar, Badge, ScrollableContainer } from "@gitbutler/ui-svelte";
	import { getTimeAgo } from "@gitbutler/ui-svelte/utils/timeAgo";
	import type { BranchIconName } from "$lib/branches/branchIcon";
	import type {
		BranchIntegrationDisplayConnectorKind,
		BranchIntegrationDisplayRailKind,
		BranchIntegrationDisplayRow,
	} from "$lib/upstream/branchIntegrationCurrentStateDisplay";
	import type {
		IntegrationGraphRef,
		IntegrationGraphRow,
		IntegrationGraphRowCommit,
	} from "$lib/upstream/branchIntegrationView";

	type BranchIntegrationGraphRow = BranchIntegrationDisplayRow | IntegrationGraphRow;
	type BranchIntegrationGraphCommitRow = Extract<BranchIntegrationGraphRow, { kind: "commit" }>;

	type Props = {
		isPreview: boolean;
		rows: BranchIntegrationGraphRow[];
		testId: string;
		showIntegratedLocalCommits?: boolean;
		toggleIntegratedLocalCommits?: (() => void) | undefined;
	};

	let {
		isPreview,
		rows,
		testId,
		showIntegratedLocalCommits = false,
		toggleIntegratedLocalCommits = undefined,
	}: Props = $props();

	function railKindClass(railKind: BranchIntegrationDisplayRailKind | undefined) {
		return railKind ?? "local";
	}

	function hasDisplayMetadata(
		row: BranchIntegrationGraphRow,
	): row is Extract<BranchIntegrationDisplayRow, { kind: "commit" }> {
		return row.kind === "commit" && "showTopConnector" in row;
	}

	function topConnectorForRow(row: BranchIntegrationGraphCommitRow, index: number) {
		return hasDisplayMetadata(row) ? row.showTopConnector : index > 0;
	}

	function topConnectorKindForRow(row: BranchIntegrationGraphCommitRow) {
		return hasDisplayMetadata(row) ? row.topConnectorKind : row.commitKind;
	}

	function leftRailKindForRow(row: BranchIntegrationGraphRow) {
		return "leftRailKind" in row ? row.leftRailKind : undefined;
	}

	function nextRowIsJoin(index: number) {
		return rows[index + 1]?.kind === "join";
	}

	function getIconFromCommitKind(
		commitKind: IntegrationGraphRowCommit["commitKind"],
	): BranchIconName {
		switch (commitKind) {
			case "remote":
				return "branch";
			case "integrated":
				return "branch-merge";
			case "local":
				return "branch-local";
		}
	}
	function getColorFromCommitKind(commitKind: IntegrationGraphRowCommit["commitKind"]): string {
		switch (commitKind) {
			case "remote":
				return "var(--commit-remote)";
			case "integrated":
				return "var(--commit-integrated)";
			case "local":
				return "var(--commit-local)";
		}
	}
</script>

<ScrollableContainer wide>
	<div class="graph">
		{#each rows as row, index (`${testId}-${index}`)}
			{#if row.kind === "collapsedIntegratedLocalSummary"}
				{@render collapsedIntegratedLocalSummaryRow(
					row,
					testId,
					showIntegratedLocalCommits,
					toggleIntegratedLocalCommits,
				)}
			{:else if row.kind === "join"}
				<!-- Rendered as a curve inside the previous commit row -->
			{:else}
				{#if row.content.refDisplays.length > 0}
					{#each row.content.refDisplays as ref, refIndex (`${ref.kind}-${ref.name}`)}
						{@render refRow(
							ref,
							row,
							refIndex > 0 || topConnectorForRow(row, index),
							refIndex > 0
								? (row.content.refDisplays[refIndex - 1]?.kind ?? row.commitKind)
								: topConnectorKindForRow(row),
						)}
					{/each}
				{/if}
				<div
					class="graph-row"
					data-testid={testId}
					data-branch-integration-row-kind={row.commitKind}
					data-branch-integration-row-commit-id={row.content.commitId}
					data-branch-integration-row-subject={row.content.subject}
				>
					{#if row.leftRail === "|"}
						<div class="graph-rail">
							<div
								class={`graph-vertical-edge graph-vertical-edge--${railKindClass(leftRailKindForRow(row))}`}
							></div>
						</div>
					{/if}
					{#if row.node === "*"}
						{@render commitNode(
							row.commitKind,
							row.content.refDisplays.length > 0 || topConnectorForRow(row, index),
							row.content.refDisplays.length > 0 ? row.commitKind : topConnectorKindForRow(row),
							nextRowIsJoin(index),
						)}
					{:else if row.node !== ""}
						<div class={`graph-node graph-node--${row.commitKind}`}>
							<span class="graph-rail-text">{row.node}</span>
						</div>
					{/if}
					{#if row.rightRail !== ""}
						<div class="graph-rail">
							<span class={`graph-rail-text graph-rail-text--${row.commitKind}`}>
								{row.rightRail}
							</span>
						</div>
					{/if}
					<div class="graph-content">
						{#if row.content.subject === ""}
							<div class="graph-subject text-13 text-semibold truncate clr-text-3">
								No commit message
							</div>
						{:else}
							<div class="graph-subject text-13 text-semibold truncate">{row.content.subject}</div>
						{/if}
						<div class="graph-meta text-12">
							{#if row.content.author}
								<div class="graph-author">
									<Avatar
										size="small"
										srcUrl={row.content.author.gravatarUrl}
										username={row.content.author.name}
										tooltip={`${row.content.author.name} (${row.content.author.email})`}
									/>
								</div>
							{/if}
							{#if !isPreview && row.content.createdAt > 0}
								<span>{getTimeAgo(row.content.createdAt)}</span>
							{/if}
							<span>{row.content.commitId.slice(0, 7)}</span>
							{#if row.content.changeId}
								<span class="metadata-separator">•</span>
								<span class="change-id">{row.content.changeId.slice(0, 4)}</span>
							{/if}
							{#if row.content.hasConflicts}
								<span class="metadata-separator">•</span>
								<Badge style="danger" kind="soft">Conflict</Badge>
							{/if}
						</div>
					</div>
				</div>
			{/if}
		{/each}
	</div>
</ScrollableContainer>

{#snippet refRow(
	ref: IntegrationGraphRef,
	row: IntegrationGraphRowCommit | BranchIntegrationGraphCommitRow,
	showTopConnector: boolean,
	topConnectorKind: BranchIntegrationDisplayConnectorKind,
)}
	{@const branchIcon = getIconFromCommitKind(ref.kind)}
	{@const branchColor = getColorFromCommitKind(ref.kind)}
	<div class="graph-row" data-testid={testId}>
		{#if row.leftRail === "|"}
			<div class="graph-rail">
				<div
					class={`graph-vertical-edge graph-vertical-edge--${railKindClass(leftRailKindForRow(row))}`}
				></div>
			</div>
		{/if}

		<div class="graph-ref-node">
			{#if showTopConnector}
				<div
					class={`graph-node-connector graph-node-connector--ref-top graph-node-connector--${topConnectorKind}`}
				></div>
			{/if}
			<BranchHeaderIcon color={branchColor} iconName={branchIcon} />
			<div
				class={`graph-node-connector graph-node-connector--bottom graph-node-connector--ref graph-node-connector--${ref.kind}`}
			></div>
		</div>

		<div class="graph-content graph-content--ref">
			<h3 class="graph-subject truncate text-14 text-bold">
				{ref.name}
			</h3>
		</div>
	</div>
{/snippet}

{#snippet collapsedIntegratedLocalSummaryRow(
	row: Extract<BranchIntegrationDisplayRow, { kind: "collapsedIntegratedLocalSummary" }>,
	testId: string,
	showIntegratedLocalCommits: boolean,
	toggleIntegratedLocalCommits: (() => void) | undefined,
)}
	<button
		type="button"
		class="graph-row graph-row--interactive"
		data-testid={testId}
		data-branch-integration-row-kind="integrated"
		data-branch-integration-row-summary="collapsed-integrated-local-commits"
		onclick={toggleIntegratedLocalCommits}
	>
		{@render commitNode("integrated", row.showTopConnector, row.topConnectorKind)}
		<div class="graph-content">
			<div class="graph-subject truncate">
				{showIntegratedLocalCommits ? "Hide" : "Show"}
				{row.hiddenCount} integrated
				{row.hiddenCount === 1 ? " commit" : " commits"}
			</div>
		</div>
	</button>
{/snippet}

{#snippet commitNode(
	commitKind: "local" | "remote" | "integrated",
	showTopConnector: boolean,
	topConnectorKind: BranchIntegrationDisplayConnectorKind,
	joinsToLeftRail: boolean = false,
)}
	<div class={`graph-node graph-node--${commitKind}`}>
		{#if showTopConnector}
			<div
				class={`graph-node-connector graph-node-connector--top graph-node-connector--${topConnectorKind}`}
			></div>
		{/if}
		<div class="graph-node-dot"></div>
		{#if joinsToLeftRail}
			<div class={`graph-node-join graph-node-join--${commitKind}`}></div>
		{:else}
			<div
				class={`graph-node-connector graph-node-connector--bottom graph-node-connector--${commitKind}`}
			></div>
		{/if}
	</div>
{/snippet}

<style lang="postcss">
	.graph {
		display: flex;
		flex-direction: column;
	}

	.graph-row {
		display: flex;
		column-gap: 4px;
		flex-shrink: 0;
		padding: 0 12px;
		border-bottom: 1px solid var(--border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.graph-content {
		display: flex;
		flex-direction: column;
		padding-block: 10px;
		padding-left: 10px;
		overflow: hidden;
		gap: 6px;
	}

	.graph-content--ref {
		padding-block: 14px;
	}

	.graph-subject {
		flex: 1;
		width: 100%;
	}

	.graph-row--interactive {
		background-color: var(--hover-purple-bg);
		cursor: pointer;
		user-select: none;
	}

	.graph-rail,
	.graph-node {
		display: flex;
		position: relative;
		flex-shrink: 0;
		justify-content: center;
		width: 18px;
		min-height: 18px;
	}

	.graph-ref-node {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 18px;
	}

	.graph-node {
		--branch-integration-node-color: var(--text-2);
	}

	.graph-node--local {
		--branch-integration-node-color: var(--commit-local);
	}

	.graph-node--remote {
		--branch-integration-node-color: var(--commit-remote);
	}

	.graph-node--integrated {
		--branch-integration-node-color: var(--commit-integrated);
	}

	.graph-rail-text {
		color: var(--text-2);
	}

	.graph-rail-text--remote {
		color: var(--hover-pop);
	}

	.graph-vertical-edge {
		width: 2px;
		height: 100%;
		background: var(--text-2);
	}

	.graph-vertical-edge--local {
		background: var(--commit-local);
	}

	.graph-vertical-edge--integrated {
		background: var(--commit-integrated);
	}

	.graph-node-join {
		box-sizing: border-box;
		position: absolute;
		top: 26px;
		right: calc(50% - 1px);
		bottom: 12px;
		width: 24px;
		border-right: 2px solid var(--branch-integration-node-color);
		border-bottom: 2px solid var(--branch-integration-node-color);
		border-bottom-right-radius: 8px;
	}

	.graph-node-join--remote {
		border-color: var(--commit-remote);
	}

	.graph-node-join--integrated {
		border-color: var(--commit-integrated);
	}

	.graph-node-dot {
		box-sizing: border-box;
		position: absolute;
		top: 13px;
		left: 50%;
		width: 10px;
		height: 10px;
		transform: translateX(-50%);
		border-radius: 10px;
		outline: 2px solid var(--bg-1);
		background-color: var(--branch-integration-node-color);
	}

	.graph-node-connector {
		position: absolute;
		left: 50%;
		width: 2px;
		transform: translateX(-50%);
		background: var(--branch-integration-node-color);
	}

	.graph-node-connector--local {
		background: var(--commit-local);
	}

	.graph-node-connector--remote {
		background: var(--commit-remote);
	}

	.graph-node-connector--integrated {
		background: var(--commit-integrated);
	}

	.graph-node-connector--top {
		top: 0;
		height: 10px;
	}

	.graph-node-connector--bottom {
		top: 26px;
		bottom: 0;
	}

	.graph-node-connector--ref {
		top: calc(50% + 14px);
	}

	.graph-node-connector--ref-top {
		top: 0;
		height: calc(50% - 14px);
	}

	.graph-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-2);
	}

	.metadata-separator {
		color: var(--text-3);
	}

	.graph-author {
		display: flex;
		align-items: center;
	}
</style>
