<script lang="ts">
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

	type ReviewChange = {
		key: string;
		label: string;
		before: string;
		after: string;
		changeKind: UnitySemanticChange["changeKind"];
		note: string;
		selection: UnitySelection;
		details: UnitySemanticChange[];
		source: UnitySemanticNode;
	};

	type UnityObjectReference = {
		fileId?: string;
		guid?: string;
		type?: string;
	};

	const INITIAL_OBJECTS = 10;
	const OBJECTS_PER_FRAME = 10;

	const { projectId, stackId, change, selectable, selectionId, onShowRaw }: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const isWorktreeSelection = $derived(selectionId.type === "worktree");
	let diff = $state<UnitySemanticDiff | null>();
	let error = $state<unknown>();
	let loading = $state(false);
	let requested = $state(true);
	let requestId = 0;
	let renderedObjectCount = $state(INITIAL_OBJECTS);

	$effect(() => {
		void projectId;
		void change;
		requested = true;
		diff = undefined;
		error = undefined;
		loading = true;
		renderedObjectCount = INITIAL_OBJECTS;
		requestId++;

		const currentRequest = requestId;
		diffService
			.fetchUnitySemanticDiff(projectId, change)
			.then((result) => {
				if (currentRequest !== requestId) return;
				diff = result;
			})
			.catch((err) => {
				if (currentRequest !== requestId) return;
				error = err;
			})
			.finally(() => {
				if (currentRequest !== requestId) return;
				loading = false;
			});

		return () => {
			diffService.cancelUnitySemanticDiff(projectId, change);
		};
	});

	$effect(() => {
		void diff?.nodes;
		renderedObjectCount = INITIAL_OBJECTS;
		const total = diff?.nodes.length ?? 0;
		if (total <= INITIAL_OBJECTS) return;

		let rafId: number;
		function addMore() {
			renderedObjectCount = Math.min(renderedObjectCount + OBJECTS_PER_FRAME, total);
			if (renderedObjectCount < total) {
				rafId = requestAnimationFrame(addMore);
			}
		}
		rafId = requestAnimationFrame(addMore);
		return () => cancelAnimationFrame(rafId);
	});

	async function loadSemanticDiff(force = false) {
		const currentRequest = ++requestId;
		requested = true;
		loading = true;
		error = undefined;

		try {
			const result = await diffService.fetchUnitySemanticDiff(projectId, change, force);
			if (currentRequest !== requestId) return;
			diff = result;
		} catch (err) {
			if (currentRequest !== requestId) return;
			error = err;
		} finally {
			if (currentRequest !== requestId) return;
			loading = false;
		}
	}

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

	function nodeKindLabel(kind: string) {
		switch (kind) {
			case "gameObject":
				return "Object";
			case "component":
				return "Component";
			case "prefabOverride":
				return "Prefab override";
			case "property":
				return "Property group";
			case "warning":
				return "Warning";
			default:
				return "Unity item";
		}
	}

	function fileKindLabel(kind: string) {
		switch (kind) {
			case "scene":
				return "scene";
			case "prefab":
				return "prefab";
			default:
				return "Unity file";
		}
	}

	function summaryText(diff: UnitySemanticDiff) {
		const parts = [];
		const reviewCount = diff.nodes.reduce(
			(total, node) => total + collectReviewChanges(node).length,
			0,
		);

		if (diff.summary.objectsChanged > 0) {
			parts.push(
				`${diff.summary.objectsChanged} object${diff.summary.objectsChanged === 1 ? "" : "s"}`,
			);
		}
		if (diff.summary.componentsChanged > 0) {
			parts.push(
				`${diff.summary.componentsChanged} component${diff.summary.componentsChanged === 1 ? "" : "s"}`,
			);
		}
		if (diff.summary.prefabOverridesChanged > 0) {
			parts.push(
				`${diff.summary.prefabOverridesChanged} prefab override${diff.summary.prefabOverridesChanged === 1 ? "" : "s"}`,
			);
		}
		if (reviewCount > 0) {
			parts.push(`${reviewCount} meaningful change${reviewCount === 1 ? "" : "s"}`);
		}

		return parts.length > 0 ? parts.join(", ") : "Context only";
	}

	function displayValue(change: UnitySemanticChange, value: string | null | undefined) {
		if (value == null || value === "") return "None";
		const objectRef = parseUnityObjectReference(value);
		if (objectRef) {
			if (change.propertyPath === "guid" || change.label.toLowerCase() === "guid") {
				return shortenId(objectRef.guid ?? value);
			}
			if (objectRef.guid) {
				return `Asset ${shortenId(objectRef.guid)}`;
			}
			if (objectRef.fileId) {
				return `File ID ${objectRef.fileId}`;
			}
			return value;
		}
		if (/^[a-f0-9]{32}$/i.test(value)) return shortenId(value);
		return value;
	}

	function changedValueLabel(change: UnitySemanticChange) {
		if (change.changeKind === "added") return "Added";
		if (change.changeKind === "removed") return "Removed";
		if (isReferenceProperty(change)) return "Reference";
		return "Changed";
	}

	function propertyLabel(change: UnitySemanticChange) {
		const path = change.propertyPath;
		const label = change.label;
		if (path === "programSource") return "Script asset";
		if (path === "guid") return "Asset GUID";
		if (path === "fileID") return "File ID";
		if (path === "m_Name" || label === "Name") return "Name";
		if (path === "m_Layer" || label === "Layer") return "Layer";
		if (path === "m_IsActive" || label === "Active state") return "Active";
		if (path === "m_Icon" || label === "Icon") return "Icon";
		if (path === "m_Component" || label === "component") return "Component reference";
		if (path === "m_CorrespondingSourceObject") return "Source prefab object";
		return label.replace(/^m_/, "");
	}

	function referencePair(change: UnitySemanticChange) {
		return {
			oldGuid: referenceGuid(change.oldValue),
			newGuid: referenceGuid(change.newValue),
			oldFileId: referenceFileId(change.oldValue),
			newFileId: referenceFileId(change.newValue),
		};
	}

	function referenceGuid(value: string | null | undefined) {
		if (!value) return undefined;
		return (
			parseUnityObjectReference(value)?.guid ?? (/^[a-f0-9]{32}$/i.test(value) ? value : undefined)
		);
	}

	function referenceFileId(value: string | null | undefined) {
		if (!value) return undefined;
		return parseUnityObjectReference(value)?.fileId;
	}

	function sameReferencePair(left: UnitySemanticChange, right: UnitySemanticChange) {
		const leftRef = referencePair(left);
		const rightRef = referencePair(right);
		return (
			!!(leftRef.oldGuid || leftRef.newGuid || leftRef.oldFileId || leftRef.newFileId) &&
			leftRef.oldGuid === rightRef.oldGuid &&
			leftRef.newGuid === rightRef.newGuid &&
			leftRef.oldFileId === rightRef.oldFileId &&
			leftRef.newFileId === rightRef.newFileId
		);
	}

	function companionReferenceChanges(
		change: UnitySemanticChange,
		changes: UnitySemanticChange[],
		consumed: Set<UnitySemanticChange>,
	) {
		if (!isReferenceProperty(change)) return [];
		return changes.filter((candidate) => {
			if (candidate === change || consumed.has(candidate)) return false;
			if (!["guid", "fileID"].includes(candidate.propertyPath)) return false;
			return sameReferencePair(change, candidate);
		});
	}

	function componentLabel(node: UnitySemanticNode) {
		if (node.kind !== "component") return node.label;
		if (node.label.startsWith("Script ")) return "Script component";
		if (node.className === "MonoBehaviour") return "Behaviour component";
		return node.label;
	}

	function nodeLabel(node: UnitySemanticNode) {
		if (node.kind === "component") return componentLabel(node);
		return node.label;
	}

	function showChangeBadge(kind: string) {
		return kind !== "unchanged";
	}

	function isReferenceProperty(change: UnitySemanticChange) {
		return (
			["programSource", "guid", "fileID", "m_CorrespondingSourceObject", "m_Component"].includes(
				change.propertyPath,
			) || !!parseUnityObjectReference(change.oldValue ?? change.newValue ?? "")
		);
	}

	function parseUnityObjectReference(value: string): UnityObjectReference | undefined {
		const fileId = value.match(/fileID:\s*([^,}\s]+)/)?.[1];
		const guid = value.match(/guid:\s*([^,}\s]+)/)?.[1];
		const type = value.match(/type:\s*([^,}\s]+)/)?.[1];
		if (!fileId && !guid && !type) return undefined;
		return { fileId, guid, type };
	}

	function shortenId(value: string) {
		if (value.length <= 12) return value;
		return `${value.slice(0, 8)}...${value.slice(-4)}`;
	}

	function reviewChanges(
		changes: UnitySemanticChange[],
		source: UnitySemanticNode,
		keyPrefix = "",
	): ReviewChange[] {
		const result: ReviewChange[] = [];
		const consumed = new Set<UnitySemanticChange>();
		const referenceChanges = changes.filter((change) => change.propertyPath === "programSource");

		for (const [index, scriptReference] of referenceChanges.entries()) {
			if (consumed.has(scriptReference)) continue;
			const details = [
				scriptReference,
				...companionReferenceChanges(scriptReference, changes, consumed),
			];
			details.forEach((detail) => consumed.add(detail));
			result.push({
				key: `${keyPrefix}script-reference:${index}`,
				label: "Script asset",
				before: displayValue(scriptReference, scriptReference.oldValue),
				after: displayValue(scriptReference, scriptReference.newValue),
				changeKind: scriptReference.changeKind,
				note: source.label.startsWith("Script ") ? "Component reference" : nodeLabel(source),
				selection: scriptReference.selection,
				details,
				source,
			});
		}

		for (const [index, change] of changes.entries()) {
			if (consumed.has(change)) continue;
			const details = [change, ...companionReferenceChanges(change, changes, consumed)];
			details.forEach((detail) => consumed.add(detail));
			result.push({
				key: `${keyPrefix}${change.propertyPath}:${index}`,
				label: propertyLabel(change),
				before: displayValue(change, change.oldValue),
				after: displayValue(change, change.newValue),
				changeKind: change.changeKind,
				note: changedValueLabel(change),
				selection: change.selection,
				details,
				source,
			});
		}
		return result;
	}

	function collectReviewChanges(node: UnitySemanticNode, prefix = ""): ReviewChange[] {
		const result = reviewChanges(node.changes, node, `${prefix}${node.id}:`);
		for (const [index, child] of node.children.entries()) {
			result.push(...collectReviewChanges(child, `${prefix}${node.id}:${index}/`));
		}
		return result;
	}

	function tooLongWarning(diff: UnitySemanticDiff | null) {
		if (!diff || diff.nodes.length > 0) return;
		return diff.warnings.find((warning) =>
			["This Unity diff is too long", "This Unity file is too large"].some((message) =>
				warning.message.startsWith(message),
			),
		);
	}

	function nodeKey(node: UnitySemanticNode, index: number, prefix = "") {
		return `${prefix}${node.id}:${index}`;
	}

	function objectSubtitle(reviewCount: number) {
		if (reviewCount === 0) return "Context only";
		return `${reviewCount} meaningful change${reviewCount === 1 ? "" : "s"}`;
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

{#snippet reviewChangeRow(item: ReviewChange)}
	<div class="unity-review-row">
		<div class="unity-review-row__select">
			{@render selectionControl(item.selection, "Include change")}
		</div>
		<div class="unity-review-row__body">
			<div class="unity-review-row__headline">
				<span>{item.label}</span>
				{#if showChangeBadge(item.changeKind)}
					<Badge kind="soft" style={changeKindStyle(item.changeKind)}>
						{changeKindLabel(item.changeKind)}
					</Badge>
				{/if}
				<span class="unity-review-row__note text-11">{item.note}</span>
			</div>
			<div class="unity-review-values">
				<div class="unity-review-value">
					<span class="unity-review-value__label text-11">Before</span>
					<span class="unity-review-value__text old">{item.before}</span>
				</div>
				<div class="unity-review-value">
					<span class="unity-review-value__label text-11">After</span>
					<span class="unity-review-value__text new">{item.after}</span>
				</div>
			</div>
			<details class="unity-details">
				<summary>Serialized values</summary>
				<code>source: {item.source.label}</code>
				{#if item.source.className}
					<code>type: {item.source.className}</code>
				{/if}
				{#if item.source.path}
					<code>path: {item.source.path}</code>
				{/if}
				{#each item.details as change}
					<code>property: {change.propertyPath}</code>
					{#if change.oldValue != null}
						<code>before: {change.oldValue}</code>
					{/if}
					{#if change.newValue != null}
						<code>after: {change.newValue}</code>
					{/if}
				{/each}
			</details>
		</div>
	</div>
{/snippet}

{#snippet objectSection(node: UnitySemanticNode, index: number, prefix = "")}
	{@const items = collectReviewChanges(node, `${prefix}${index}/`)}
	<section class="unity-object">
		<div class="unity-object__header">
			<div class="unity-object__select">
				{@render selectionControl(node.selection, nodeSelectionLabel(node.kind))}
			</div>
			<div class="unity-object__title-block">
				<div class="unity-object__title">
					<span class="unity-row-kind text-11">{nodeKindLabel(node.kind)}</span>
					<span>{nodeLabel(node)}</span>
					{#if showChangeBadge(node.changeKind)}
						<Badge kind="soft" style={changeKindStyle(node.changeKind)}>
							{changeKindLabel(node.changeKind)}
						</Badge>
					{/if}
				</div>
				<div class="unity-object__meta text-11">
					<span>{objectSubtitle(items.length)}</span>
				</div>
			</div>
		</div>

		{#if items.length > 0}
			<div class="unity-review-list">
				{#each items as item (item.key)}
					{@render reviewChangeRow(item)}
				{/each}
			</div>
		{/if}
	</section>
{/snippet}

{#snippet semanticContent(diff: UnitySemanticDiff | null)}
	{@const tooLong = tooLongWarning(diff)}
	{#if tooLong}
		<div class="unity-warning unity-warning--too-large">
			<p class="text-13 text-semibold">This Unity diff is too large</p>
			<p class="text-12 clr-text-2">{tooLong.message}</p>
			{#if onShowRaw}
				<Button kind="outline" size="tag" onclick={onShowRaw}>Show raw diff</Button>
			{/if}
		</div>
	{:else if !diff || diff.nodes.length === 0}
		<div class="unity-empty">
			<p class="text-13 text-semibold">Unity view is not available for this change.</p>
			<p class="text-12 clr-text-2">Raw diff is still available.</p>
			{#if onShowRaw}
				<Button kind="outline" size="tag" onclick={onShowRaw}>Show raw diff</Button>
			{/if}
		</div>
	{:else}
		<div class="unity-summary">
			<div>
				<p class="text-13 text-semibold">Unity {fileKindLabel(diff.fileKind)} review</p>
				<p class="text-12 clr-text-2">
					Grouped by object and component. Open serialized values only when you need the raw Unity
					values.
				</p>
			</div>
			<div class="unity-summary__badges">
				<Badge kind="soft" style="gray">{summaryText(diff)}</Badge>
			</div>
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
			{#each diff.nodes.slice(0, renderedObjectCount) as node, index (nodeKey(node, index))}
				{@render objectSection(node, index)}
			{/each}
			{#if renderedObjectCount < diff.nodes.length}
				<div class="unity-empty unity-empty--loading-more">
					<p class="text-12 clr-text-2">Loading Unity objects...</p>
				</div>
			{/if}
		</div>
	{/if}
{/snippet}

{#if !requested}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Unity semantic view is paused.</p>
		<p class="text-12 clr-text-2">Load it when you need object-level review.</p>
		<Button kind="outline" size="tag" onclick={() => loadSemanticDiff()}>Load Unity view</Button>
		{#if onShowRaw}
			<Button kind="ghost" size="tag" onclick={onShowRaw}>Show raw diff</Button>
		{/if}
	</div>
{:else if loading}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Reading Unity scene...</p>
		{#if onShowRaw}
			<Button kind="ghost" size="tag" onclick={onShowRaw}>Show raw diff</Button>
		{/if}
	</div>
{:else if error}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Unity view failed to load.</p>
		<p class="text-12 clr-text-2">{String(error)}</p>
		<Button kind="outline" size="tag" onclick={() => loadSemanticDiff(true)}>Try again</Button>
		{#if onShowRaw}
			<Button kind="ghost" size="tag" onclick={onShowRaw}>Show raw diff</Button>
		{/if}
	</div>
{:else}
	{@render semanticContent(diff ?? null)}
{/if}

<style lang="postcss">
	.unity-summary {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		padding: 2px 0 0;
		gap: 12px;
	}

	.unity-summary p {
		margin: 0;
	}

	.unity-summary__badges {
		display: flex;
		flex-shrink: 0;
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

	.unity-warning--too-large {
		border-style: dashed;
	}

	.unity-empty--loading-more {
		align-items: center;
	}

	.unity-tree {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.unity-object {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-0);
	}

	.unity-object__header {
		display: flex;
		align-items: flex-start;
		padding: 10px 12px;
		gap: 10px;
		background-color: var(--bg-1);
	}

	.unity-object__select {
		flex-shrink: 0;
		width: 16px;
	}

	.unity-object__title-block {
		flex: 1;
		min-width: 0;
	}

	.unity-object__title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-1);
		font-weight: 600;
	}

	.unity-object__meta {
		display: flex;
		flex-wrap: wrap;
		margin-top: 4px;
		gap: 8px;
		color: var(--text-3);
	}

	.unity-row-kind {
		padding: 1px 5px;
		border-radius: var(--radius-s);
		background-color: var(--bg-3);
		color: var(--text-2);
		font-weight: 600;
		text-transform: uppercase;
	}

	.unity-review-list {
		display: flex;
		flex-direction: column;
		padding-left: 26px;
	}

	.unity-review-row {
		display: flex;
		padding: 10px 12px 10px 0;
		gap: 10px;
		box-shadow: inset 0 1px 0 var(--border-3);
	}

	.unity-review-row:first-child {
		box-shadow: inset 0 1px 0 var(--border-2);
	}

	.unity-review-row__select {
		flex-shrink: 0;
		width: 16px;
	}

	.unity-review-row__body {
		flex: 1;
		min-width: 0;
	}

	.unity-review-row__headline {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-1);
		font-weight: 600;
	}

	.unity-review-row__note,
	.unity-review-value__label {
		color: var(--text-3);
	}

	.unity-review-values {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
		margin-top: 6px;
		gap: 12px;
	}

	.unity-review-value {
		display: flex;
		flex-direction: column;
		min-width: 0;
		gap: 2px;
	}

	.unity-review-value__text {
		overflow: hidden;
		font-weight: 500;
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.old {
		color: var(--clr-scale-red-500);
	}

	.new {
		color: var(--clr-scale-green-500);
	}

	.unity-details {
		margin-top: 6px;
		color: var(--text-3);
		font-size: 11px;

		summary {
			width: fit-content;
			color: var(--text-2);
			cursor: pointer;
		}

		summary:hover {
			color: var(--text-1);
		}

		code {
			display: block;
			margin-top: 4px;
			overflow-wrap: anywhere;
		}
	}
</style>
