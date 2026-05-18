<script lang="ts">
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Button, Checkbox, Tooltip } from "@gitbutler/ui";
	import type {
		UnitySemanticChange,
		UnitySemanticDiff,
		UnitySemanticNode,
		UnitySelection,
	} from "$lib/files/unitySemantic";
	import type { SelectionId } from "$lib/selection/key";
	import type { TreeChange } from "@gitbutler/but-sdk";

	type Props = {
		projectId: string;
		stackId?: string;
		change: TreeChange;
		selectable: boolean;
		selectionId: SelectionId;
		onShowRaw?: () => void;
	};

	const { projectId, stackId, change, selectable, selectionId, onShowRaw }: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const semanticQuery = $derived(diffService.getUnitySemanticDiff(projectId, change));
	const isWorktreeSelection = $derived(selectionId.type === "worktree");

	function nodeSelectionLabel(kind: string) {
		switch (kind) {
			case "gameObject":
				return "Include object";
			case "component":
				return "Include component";
			case "prefabOverride":
				return "Include override";
			default:
				return "Include change";
		}
	}

	function selectionChecked(selection: UnitySelection) {
		if (selection.mode === "file") {
			return (
				uncommittedService.fileCheckStatus(stackId || undefined, change.path).current !==
				"unchecked"
			);
		}
		if (selection.hunks.length === 0) return false;
		return selection.hunks.every(
			(hunk) => uncommittedService.hunkCheckStatus(stackId, change.path, hunk).current.selected,
		);
	}

	function setSelection(selection: UnitySelection, checked: boolean) {
		if (!selectable || !isWorktreeSelection || selection.mode === "unavailable") return;
		const stack = stackId || null;
		if (selection.mode === "file") {
			if (checked) {
				uncommittedService.checkFile(stack, change.path);
			} else {
				uncommittedService.uncheckFile(stack, change.path);
			}
			return;
		}

		for (const hunk of selection.hunks) {
			if (checked) {
				if (selection.mode === "precise" && hunk.lines.length > 0) {
					for (const line of hunk.lines) {
						uncommittedService.checkLine(stack, change.path, hunk, line);
					}
				} else {
					uncommittedService.checkHunk(stack, change.path, hunk);
				}
			} else {
				uncommittedService.uncheckHunk(stack, change.path, hunk);
			}
		}
	}

	function changeKindLabel(kind: string) {
		switch (kind) {
			case "added":
				return "Added";
			case "removed":
				return "Removed";
			case "moved":
				return "Moved";
			case "modified":
				return "Modified";
			case "unchanged":
				return "Context";
			default:
				return "Changed";
		}
	}

	function changeKindStyle(kind: string) {
		switch (kind) {
			case "added":
				return "safe";
			case "removed":
				return "danger";
			case "moved":
				return "purple";
			default:
				return "gray";
		}
	}
</script>

{#snippet selectionControl(selection: UnitySelection, label: string)}
	{#if selectable && isWorktreeSelection}
		{@const disabled = selection.mode === "unavailable"}
		<Tooltip text={disabled ? "Select this from Raw diff" : label}>
			<Checkbox
				small
				checked={selectionChecked(selection)}
				{disabled}
				onchange={(event) => setSelection(selection, event.currentTarget.checked)}
			/>
		</Tooltip>
	{/if}
{/snippet}

{#snippet changeRow(change: UnitySemanticChange)}
	<div class="unity-change-row">
		{@render selectionControl(change.selection, "Include field")}
		<div class="unity-change-row__body">
			<div class="unity-change-row__title">
				<span>{change.label}</span>
				<Badge kind="soft" style={changeKindStyle(change.changeKind)}>
					{changeKindLabel(change.changeKind)}
				</Badge>
			</div>
			<div class="unity-change-row__values text-12">
				{#if change.oldValue != null}
					<span class="old">{change.oldValue}</span>
				{/if}
				{#if change.oldValue != null || change.newValue != null}
					<span class="arrow">-&gt;</span>
				{/if}
				{#if change.newValue != null}
					<span class="new">{change.newValue}</span>
				{/if}
			</div>
			<details class="unity-details">
				<summary>Details</summary>
				<code>{change.propertyPath}</code>
			</details>
		</div>
	</div>
{/snippet}

{#snippet nodeRow(node: UnitySemanticNode, depth = 0)}
	<div class="unity-node" style={`--depth: ${depth}`}>
		<div class="unity-node__header">
			{@render selectionControl(node.selection, nodeSelectionLabel(node.kind))}
			<div class="unity-node__main">
				<div class="unity-node__title">
					<span>{node.label}</span>
					<Badge kind="soft" style={changeKindStyle(node.changeKind)}>
						{changeKindLabel(node.changeKind)}
					</Badge>
					{#if node.className}
						<span class="unity-node__class text-11">{node.className}</span>
					{/if}
				</div>
				<p class="unity-node__path text-11 clr-text-2">{node.path}</p>
			</div>
		</div>
		{#if node.changes.length > 0}
			<div class="unity-node__changes">
				{#each node.changes as change}
					{@render changeRow(change)}
				{/each}
			</div>
		{/if}
		{#if node.children.length > 0}
			<div class="unity-node__children">
				{#each node.children as child (child.id)}
					{@render nodeRow(child, depth + 1)}
				{/each}
			</div>
		{/if}
	</div>
{/snippet}

{#snippet semanticContent(diff: UnitySemanticDiff | null)}
	{#if !diff || diff.nodes.length === 0}
		<div class="unity-empty">
			<p class="text-13 text-semibold">Unity view is not available for this change.</p>
			<p class="text-12 clr-text-2">Raw diff is still available.</p>
			{#if onShowRaw}
				<Button kind="outline" size="tag" onclick={onShowRaw}>Show raw diff</Button>
			{/if}
		</div>
	{:else}
		<div class="unity-summary">
			<Badge kind="soft" style="gray">{diff.summary.objectsChanged} objects</Badge>
			<Badge kind="soft" style="gray">{diff.summary.componentsChanged} components</Badge>
			<Badge kind="soft" style="gray">{diff.summary.prefabOverridesChanged} overrides</Badge>
			<Badge kind="soft" style="gray">{diff.summary.propertiesChanged} fields</Badge>
		</div>
		{#if diff.warnings.length > 0}
			<div class="unity-warning">
				<p class="text-12 text-semibold">Unity view is partial</p>
				{#each diff.warnings as warning}
					<p class="text-12 clr-text-2">
						{warning.message}{warning.line ? ` near line ${warning.line}` : ""}
					</p>
				{/each}
			</div>
		{/if}
		<div class="unity-tree">
			{#each diff.nodes as node (node.id)}
				{@render nodeRow(node)}
			{/each}
		</div>
	{/if}
{/snippet}

<ReduxResult {projectId} result={semanticQuery.result}>
	{#snippet children(diff)}
		{@render semanticContent(diff)}
	{/snippet}
	{#snippet loading()}
		<div class="unity-empty">
			<p class="text-13 text-semibold">Reading Unity scene...</p>
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.unity-summary {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.unity-warning,
	.unity-empty {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 6px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-2);
	}

	.unity-tree {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.unity-node {
		display: flex;
		flex-direction: column;
		padding-left: calc(var(--depth) * 14px);
		gap: 8px;
	}

	.unity-node__header,
	.unity-change-row {
		display: flex;
		align-items: flex-start;
		padding: 10px;
		gap: 10px;
		border: 1px solid var(--border-3);
		border-radius: var(--radius-m);
		background-color: var(--bg-0);
	}

	.unity-node__main,
	.unity-change-row__body {
		flex: 1;
		min-width: 0;
	}

	.unity-node__title,
	.unity-change-row__title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-1);
		font-weight: 600;
	}

	.unity-node__class {
		color: var(--text-3);
	}

	.unity-node__path {
		margin-top: 2px;
		word-break: break-word;
	}

	.unity-node__changes,
	.unity-node__children {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.unity-change-row {
		margin-left: 24px;
		background-color: var(--bg-1);
	}

	.unity-change-row__values {
		display: flex;
		flex-wrap: wrap;
		margin-top: 4px;
		gap: 6px;
		word-break: break-word;
	}

	.old {
		color: var(--clr-scale-red-500);
	}

	.new {
		color: var(--clr-scale-green-500);
	}

	.arrow {
		color: var(--text-3);
	}

	.unity-details {
		margin-top: 6px;
		color: var(--text-3);
		font-size: 11px;

		code {
			display: block;
			margin-top: 4px;
			overflow-wrap: anywhere;
		}
	}
</style>
