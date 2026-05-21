<script lang="ts">
	import { Avatar } from "@gitbutler/ui";
	import { getTimeAgo } from "@gitbutler/ui/utils/timeAgo";
	import type { BranchIntegrationDisplayRow } from "$lib/upstream/branchIntegrationCurrentStateDisplay";

	type Props = {
		rows: BranchIntegrationDisplayRow[];
		testId: string;
		showIntegratedLocalCommits?: boolean;
		toggleIntegratedLocalCommits?: (() => void) | undefined;
	};

	let {
		rows,
		testId,
		showIntegratedLocalCommits = false,
		toggleIntegratedLocalCommits = undefined,
	}: Props = $props();
</script>

<div class="branch-integration__graph">
	{#each rows as row, index (`${testId}-${index}`)}
		{#if row.kind === "collapsedIntegratedLocalSummary"}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div
				class="branch-integration__graph-row branch-integration__graph-row--interactive"
				data-testid={testId}
				data-branch-integration-row-kind="integrated"
				data-branch-integration-row-summary="collapsed-integrated-local-commits"
				onclick={toggleIntegratedLocalCommits}
			>
				<div class="branch-integration__graph-node branch-integration__graph-node--integrated">
					<div
						class="branch-integration__graph-node-dot branch-integration__graph-node-dot--integrated"
					></div>
				</div>
				<div
					class="branch-integration__graph-rail branch-integration__graph-rail--integrated"
				></div>
				<div class="branch-integration__graph-content">
					<div class="branch-integration__graph-subject">
						{showIntegratedLocalCommits ? "Hide" : "Show"}
						{row.hiddenCount} integrated
						{row.hiddenCount === 1 ? " commit" : " commits"}
					</div>
				</div>
			</div>
		{:else if row.kind === "join"}
			<div
				class="branch-integration__graph-row branch-integration__graph-row--join"
				data-testid={testId}
				data-branch-integration-row-kind="join"
			>
				{#if row.leftRail === "|"}
					<div class="branch-integration__graph-rail">
						<div class="branch-integration__graph-vertical-edge"></div>
					</div>
				{/if}
				{#if row.node !== ""}
					<div class="branch-integration__graph-node">
						<span class="branch-integration__graph-rail-text">{row.node}</span>
					</div>
				{/if}
				<div class="branch-integration__graph-rail">
					{#if row.rightRail !== ""}
						<div class="branch-integration__graph-remote-join" aria-hidden="true">
							<div class="branch-integration__graph-remote-join-horizontal"></div>
							<div class="branch-integration__graph-remote-join-vertical"></div>
						</div>
					{/if}
				</div>
				<div></div>
			</div>
		{:else}
			<div
				class="branch-integration__graph-row"
				data-testid={testId}
				data-branch-integration-row-kind={row.commitKind}
				data-branch-integration-row-commit-id={row.content.commitId}
				data-branch-integration-row-subject={row.content.subject}
			>
				{#if row.leftRail === "|"}
					<div
						class={`branch-integration__graph-rail branch-integration__graph-rail--${row.commitKind}`}
					>
						<div
							class={`branch-integration__graph-vertical-edge branch-integration__graph-vertical-edge--${row.commitKind}`}
						></div>
					</div>
				{/if}
				<div
					class={`branch-integration__graph-node branch-integration__graph-node--${row.commitKind}`}
				>
					{#if row.node === "*"}
						<div
							class={`branch-integration__graph-node-dot branch-integration__graph-node-dot--${row.commitKind}`}
						></div>
					{:else if row.node !== ""}
						<span class="branch-integration__graph-rail-text">{row.node}</span>
					{/if}
				</div>
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
						{#if row.content.createdAt > 0}
							<span>{getTimeAgo(row.content.createdAt)}</span>
						{/if}
						<span>{row.content.commitId.slice(0, 7)}</span>
						{#if row.content.changeId}
							<span>•</span>
							<span class="branch-integration__change-id">{row.content.changeId.slice(0, 4)}</span>
						{/if}
						{#if row.content.refs.length > 0}
							<span>•</span>
							<span>{row.content.refs.join(", ")}</span>
						{/if}
					</div>
				</div>
			</div>
		{/if}
	{/each}
</div>

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
		align-items: center;
		justify-content: center;
		min-height: 18px;
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
		margin-left: 4px;
		background: var(--text-2);
	}

	.branch-integration__graph-remote-join {
		position: relative;
		width: 18px;
		height: 100%;
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
		width: 11px;
		height: 11px;
		border: 2px solid var(--text-2);
		border-radius: 999px;
	}

	.branch-integration__graph-node-dot--remote {
		border-color: var(--commit-remote);
	}

	.branch-integration__graph-node-dot--integrated {
		border-color: var(--commit-integrated);
	}

	.branch-integration__graph-rail--local,
	.branch-integration__graph-rail--integrated,
	.branch-integration__graph-vertical-edge--local,
	.branch-integration__graph-vertical-edge--remote,
	.branch-integration__graph-vertical-edge--integrated,
	.branch-integration__graph-node--remote,
	.branch-integration__graph-node--integrated,
	.branch-integration__graph-node-dot--remote,
	.branch-integration__graph-rail-text--local,
	.branch-integration__graph-rail-text--remote,
	.branch-integration__graph-rail-text--integrated {
	}

	.branch-integration__graph-content {
		display: flex;
		flex-direction: column;
		justify-content: center;
		min-width: 0;
		gap: 2px;
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
</style>
