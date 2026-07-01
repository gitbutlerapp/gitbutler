<script lang="ts">
	import BranchHeaderIcon from "$components/branch/BranchHeaderIcon.svelte";
	import { Avatar } from "@gitbutler/ui";
	import { getTimeAgo } from "@gitbutler/ui/utils/timeAgo";
	import type { BranchIconName } from "$lib/branches/branchIcon";
	import type {
		BranchIntegrationDisplayConnectorKind,
		BranchIntegrationDisplayRailKind,
		BranchIntegrationDisplayRow,
		BranchIntegrationDisplayRowJoin,
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

<div class="branch-integration__graph">
	{#each rows as row, index (`${testId}-${index}`)}
		{#if row.kind === "collapsedIntegratedLocalSummary"}
			{@render collapsedIntegratedLocalSummaryRow(
				row,
				testId,
				showIntegratedLocalCommits,
				toggleIntegratedLocalCommits,
			)}
		{:else if row.kind === "join"}
			{@render joinRow(row, testId)}
		{:else}
			{#if row.content.refDisplays.length > 0}
				{#each row.content.refDisplays as ref (`${ref.kind}-${ref.name}`)}
					{@render refRow(ref, row)}
				{/each}
			{/if}
			<div
				class="branch-integration__graph-row"
				data-testid={testId}
				data-branch-integration-row-kind={row.commitKind}
				data-branch-integration-row-commit-id={row.content.commitId}
				data-branch-integration-row-subject={row.content.subject}
			>
				{#if row.leftRail === "|"}
					<div
						class={`branch-integration__graph-rail branch-integration__graph-rail--${railKindClass(leftRailKindForRow(row))}`}
					>
						<div
							class={`branch-integration__graph-vertical-edge branch-integration__graph-vertical-edge--${railKindClass(leftRailKindForRow(row))}`}
						></div>
					</div>
				{/if}
				{#if row.node === "*"}
					{@render commitNode(
						row.commitKind,
						topConnectorForRow(row, index),
						topConnectorKindForRow(row),
					)}
				{:else if row.node !== ""}
					<div
						class={`branch-integration__graph-node branch-integration__graph-node--${row.commitKind}`}
					>
						<span class="branch-integration__graph-rail-text">{row.node}</span>
					</div>
				{/if}
				<div
					class={`branch-integration__graph-rail branch-integration__graph-rail--${row.commitKind}`}
				>
					{#if row.rightRail !== ""}
						<span
							class={`branch-integration__graph-rail-text branch-integration__graph-rail-text--${row.commitKind}`}
						>
							{row.rightRail}
						</span>
					{/if}
				</div>
				<div class="branch-integration__graph-content">
					<div class="branch-integration__graph-subject">{row.content.subject}</div>
					<div class="branch-integration__graph-meta">
						{#if row.content.author}
							<div class="branch-integration__graph-author">
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
							<span>•</span>
							<span class="branch-integration__change-id">{row.content.changeId.slice(0, 4)}</span>
						{/if}
						{#if row.content.hasConflicts}
							<span>•</span>
							<span class="branch-integration__conflict">conflict</span>
						{/if}
					</div>
				</div>
			</div>
		{/if}
	{/each}
</div>

{#snippet refRow(
	ref: IntegrationGraphRef,
	row: IntegrationGraphRowCommit | BranchIntegrationGraphCommitRow,
)}
	{@const branchIcon = getIconFromCommitKind(ref.kind)}
	{@const branchColor = getColorFromCommitKind(ref.kind)}
	<div class="branch-integration__graph-row" data-testid={testId}>
		{#if row.leftRail === "|"}
			<div
				class={`branch-integration__graph-rail branch-integration__graph-rail--${railKindClass(leftRailKindForRow(row))}`}
			>
				<div
					class={`branch-integration__graph-vertical-edge branch-integration__graph-vertical-edge--${railKindClass(leftRailKindForRow(row))}`}
				></div>
			</div>
		{/if}
		<div class="branch-integration__graph-content">
			<div class="branch-integration__graph-content--ref">
				<BranchHeaderIcon color={branchColor} iconName={branchIcon} />
				<div class="branch-integration__graph-subject">
					{ref.name}
				</div>
			</div>
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
		class="branch-integration__graph-row branch-integration__graph-row--interactive"
		data-testid={testId}
		data-branch-integration-row-kind="integrated"
		data-branch-integration-row-summary="collapsed-integrated-local-commits"
		onclick={toggleIntegratedLocalCommits}
	>
		{@render commitNode("integrated", row.showTopConnector, row.topConnectorKind)}
		<div class="branch-integration__graph-rail branch-integration__graph-rail--integrated"></div>
		<div class="branch-integration__graph-content">
			<div class="branch-integration__graph-subject">
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
)}
	<div class={`branch-integration__graph-node branch-integration__graph-node--${commitKind}`}>
		{#if showTopConnector}
			<div
				class={`branch-integration__graph-node-connector branch-integration__graph-node-connector--top branch-integration__graph-node-connector--${topConnectorKind}`}
			></div>
		{/if}
		<div
			class={`branch-integration__graph-node-dot branch-integration__graph-node-dot--${commitKind}`}
		></div>
		<div
			class={`branch-integration__graph-node-connector branch-integration__graph-node-connector--bottom branch-integration__graph-node-connector--${commitKind}`}
		></div>
	</div>
{/snippet}

{#snippet joinRow(row: BranchIntegrationDisplayRowJoin, testId: string)}
	<div
		class="branch-integration__graph-row branch-integration__graph-row--join"
		data-testid={testId}
		data-branch-integration-row-kind="join"
	>
		{#if row.leftRail === "|"}
			<div
				class={`branch-integration__graph-rail branch-integration__graph-rail--join branch-integration__graph-rail--${railKindClass(row.leftRailKind)}`}
			>
				<div
					class={`branch-integration__graph-vertical-edge branch-integration__graph-vertical-edge--${railKindClass(row.leftRailKind)}`}
				></div>
			</div>
		{/if}
		{#if row.node !== ""}
			<div class="branch-integration__graph-node">
				<span class="branch-integration__graph-rail-text">{row.node}</span>
			</div>
		{/if}
		<div class="branch-integration__graph-rail branch-integration__graph-rail--join--remote">
			{#if row.rightRail !== ""}
				<div class="branch-integration__graph-remote-join"></div>
			{/if}
		</div>
		<div></div>
	</div>
{/snippet}

<style lang="postcss">
	.branch-integration__graph {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: scroll;
	}

	.branch-integration__graph-row {
		display: flex;
		column-gap: 4px;
		flex-shrink: 0;
		align-items: stretch;
		height: 65px;
		padding: 0 14px;
		border-bottom: 1px solid var(--border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.branch-integration__graph-row--join {
		column-gap: 0;
		align-items: unset;
		gap: 0;
	}

	.branch-integration__graph-row--interactive {
		background-color: var(--hover-purple-bg);
		cursor: pointer;
		user-select: none;
	}

	.branch-integration__graph-rail,
	.branch-integration__graph-node {
		display: flex;
		position: relative;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 18px;
		min-height: 18px;
	}

	.branch-integration__graph-rail--join {
		justify-content: end;
		width: 9px;
		margin-left: 1px;
	}

	.branch-integration__graph-rail--join--remote {
		width: 22px;
		margin: 0;
		padding: 0;
	}

	.branch-integration__graph-node {
		--branch-integration-node-color: var(--text-2);
	}

	.branch-integration__graph-node--local {
		--branch-integration-node-color: var(--commit-local);
	}

	.branch-integration__graph-node--remote {
		--branch-integration-node-color: var(--commit-remote);
	}

	.branch-integration__graph-node--integrated {
		--branch-integration-node-color: var(--commit-integrated);
	}

	.branch-integration__graph-rail-text {
		color: var(--text-2);
		font-family: var(--font-mono, monospace);
		white-space: pre;

		&.remote {
			color: var(--hover-pop);
		}
	}

	.branch-integration__graph-vertical-edge {
		width: 2px;
		height: 100%;
		background: var(--text-2);
	}

	.branch-integration__graph-vertical-edge--local {
		background: var(--commit-local);
	}

	.branch-integration__graph-vertical-edge--integrated {
		background: var(--commit-integrated);
	}

	.branch-integration__graph-remote-join {
		position: relative;
		width: 100%;
		height: 100%;
		border-right: 2px solid var(--commit-remote);
		border-bottom: 2px solid var(--commit-remote);
		border-bottom-right-radius: 8px;
	}

	.branch-integration__graph-remote-join-horizontal,
	.branch-integration__graph-remote-join-vertical {
		position: absolute;
		background: var(--commit-remote);
	}

	.branch-integration__graph-remote-join-horizontal {
		right: 7px;
		bottom: 7px;
		left: 0;
		height: 2px;
	}

	.branch-integration__graph-remote-join-vertical {
		top: 0;
		right: 7px;
		bottom: 7px;
		width: 2px;
	}

	.branch-integration__graph-node-dot {
		box-sizing: border-box;
		z-index: 1;
		position: relative;
		width: 11px;
		height: 11px;
		border: 2px solid var(--branch-integration-node-color);
		border-radius: 999px;
	}

	.branch-integration__graph-node-connector {
		position: absolute;
		left: 50%;
		width: 2px;
		transform: translateX(-50%);
		background: var(--branch-integration-node-color);
	}

	.branch-integration__graph-node-connector--local {
		background: var(--commit-local);
	}

	.branch-integration__graph-node-connector--remote {
		background: var(--commit-remote);
	}

	.branch-integration__graph-node-connector--integrated {
		background: var(--commit-integrated);
	}

	.branch-integration__graph-node-connector--top {
		top: 0;
		bottom: calc(50% + 6px);
	}

	.branch-integration__graph-node-connector--bottom {
		top: calc(50% + 6px);
		bottom: 0;
	}

	.branch-integration__graph-content--ref,
	.branch-integration__graph-content {
		display: flex;
		flex-direction: column;
		justify-content: center;
		min-width: 0;
		gap: 2px;
	}

	.branch-integration__graph-content--ref {
		flex-direction: row;
		gap: 22px;
	}

	.branch-integration__graph-subject {
		overflow: hidden;
		font-weight: 600;
		font-size: 13px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.branch-integration__graph-meta {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-2);
		font-size: 11px;
	}

	.branch-integration__graph-author {
		display: flex;
		align-items: center;
	}

	.branch-integration__change-id {
		font-weight: bold;
	}
	.branch-integration__conflict {
		padding: 1px 4px;
		border-radius: 4px;
		background-color: var(--bg-warn);
		color: var(--fill-warn-bg);
	}
</style>
