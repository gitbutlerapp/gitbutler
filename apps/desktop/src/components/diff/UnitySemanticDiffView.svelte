<script lang="ts">
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Button, Checkbox, Icon, Tooltip } from "@gitbutler/ui";
	import type {
		UnitySemanticChange,
		UnitySemanticDiff,
		UnitySemanticNode,
		UnitySelection,
		UnityAssetReference,
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
	let collapsedNodes = $state<Set<string>>(new Set());

	$effect(() => {
		void projectId;
		void change;
		requested = true;
		diff = undefined;
		error = undefined;
		loading = true;
		renderedObjectCount = INITIAL_OBJECTS;
		collapsedNodes = new Set();
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

	function displayValue(
		change: UnitySemanticChange,
		value: string | null | undefined,
		reference?: UnityAssetReference | null,
	) {
		if (value == null || value === "") return "None";
		if (reference) return reference.name;
		const objectRef = parseUnityObjectReference(value);
		if (objectRef) {
			if (change.propertyPath === "guid" || change.label.toLowerCase() === "guid") {
				return shortenId(objectRef.guid ?? value);
			}
			if (objectRef.guid) {
				return `Unresolved asset (${shortenId(objectRef.guid)})`;
			}
			if (objectRef.fileId) {
				return `File ID ${objectRef.fileId}`;
			}
			return value;
		}
		if (/^[a-f0-9]{32}$/i.test(value)) return `Unresolved asset (${shortenId(value)})`;
		return value;
	}

	function changedValueLabel(change: UnitySemanticChange) {
		if (change.changeKind === "added") return "Added";
		if (change.changeKind === "removed") return "Removed";
		if (isReferenceProperty(change)) return "";
		return "Changed";
	}

	function propertyLabel(change: UnitySemanticChange) {
		const path = change.propertyPath;
		const label = change.label;
		if (path.endsWith("programSource")) return "Program source";
		if (path.endsWith("guid")) return "Asset reference";
		if (path.endsWith("fileID")) return "File ID";
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
			if (!candidate.propertyPath.endsWith("guid") && !candidate.propertyPath.endsWith("fileID")) {
				return false;
			}
			return sameReferencePair(change, candidate);
		});
	}

	function componentLabel(node: UnitySemanticNode) {
		if (node.kind !== "component") return node.label;
		if (node.label.startsWith("Script ")) {
			return `Script ${shortenId(node.label.slice("Script ".length).trim())}`;
		}
		return node.label;
	}

	function nodeLabel(node: UnitySemanticNode) {
		if (node.kind === "component") return componentLabel(node);
		return node.label;
	}

	function nodePathSegments(node: UnitySemanticNode) {
		return node.path
			.split("/")
			.map((segment) => segment.trim())
			.filter(Boolean);
	}

	function nodeDisplayName(node: UnitySemanticNode) {
		const segments = nodePathSegments(node);
		return segments.at(-1) || nodeLabel(node);
	}

	function nodePathContext(node: UnitySemanticNode) {
		const segments = nodePathSegments(node);
		if (segments.length <= 1) return "";
		return segments.slice(0, -1).join(" / ");
	}

	function showChangeBadge(kind: string) {
		return kind !== "unchanged";
	}

	function isReferenceProperty(change: UnitySemanticChange) {
		return (
			["programSource", "guid", "fileID", "m_CorrespondingSourceObject", "m_Component"].includes(
				change.propertyPath,
			) ||
			change.propertyPath.endsWith("programSource") ||
			change.propertyPath.endsWith("guid") ||
			change.propertyPath.endsWith("fileID") ||
			!!parseUnityObjectReference(change.oldValue ?? change.newValue ?? "")
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
		const referenceChanges = changes.filter((change) =>
			change.propertyPath.endsWith("programSource"),
		);
		if (referenceChanges.length > 0) {
			for (const change of changes) {
				if (change.propertyPath.endsWith("guid") || change.propertyPath.endsWith("fileID")) {
					consumed.add(change);
				}
			}
		}

		for (const [index, scriptReference] of referenceChanges.entries()) {
			if (consumed.has(scriptReference)) continue;
			const details = [
				scriptReference,
				...companionReferenceChanges(scriptReference, changes, consumed),
			];
			details.forEach((detail) => consumed.add(detail));
			result.push({
				key: `${keyPrefix}script-reference:${index}`,
				label: "Script reference changed",
				before: displayValue(
					scriptReference,
					scriptReference.oldValue,
					scriptReference.oldReference,
				),
				after: displayValue(
					scriptReference,
					scriptReference.newValue,
					scriptReference.newReference,
				),
				changeKind: scriptReference.changeKind,
				note: "",
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
				before: displayValue(change, change.oldValue, change.oldReference),
				after: displayValue(change, change.newValue, change.newReference),
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
		const result = nodeReviewChanges(node, prefix);
		for (const [index, child] of node.children.entries()) {
			result.push(...collectReviewChanges(child, `${prefix}${node.id}:${index}/`));
		}
		return dedupeReviewChanges(result);
	}

	function nodeReviewChanges(node: UnitySemanticNode, prefix = "") {
		return dedupeReviewChanges(reviewChanges(node.changes, node, `${prefix}${node.id}:`));
	}

	function dedupeReviewChanges(items: ReviewChange[]) {
		const seenReferencePairs = new Set<string>();
		return items.filter((item) => {
			const referenceKey = reviewReferenceKey(item);
			if (!referenceKey) return true;
			if (item.label !== "Script reference changed" && seenReferencePairs.has(referenceKey)) {
				return false;
			}
			seenReferencePairs.add(referenceKey);
			return true;
		});
	}

	function reviewReferenceKey(item: ReviewChange) {
		const referenceChange = item.details.find(isReferenceProperty);
		if (!referenceChange) return undefined;
		const pair = referencePair(referenceChange);
		if (!pair.oldGuid && !pair.newGuid && !pair.oldFileId && !pair.newFileId) return undefined;
		return [pair.oldGuid, pair.newGuid, pair.oldFileId, pair.newFileId].join("->");
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

	function nodeCollapseKey(node: UnitySemanticNode, index: number, prefix = "") {
		return nodeKey(node, index, prefix);
	}

	function isNodeCollapsed(key: string) {
		return collapsedNodes.has(key);
	}

	function toggleNodeCollapsed(key: string) {
		const next = new Set(collapsedNodes);
		if (next.has(key)) {
			next.delete(key);
		} else {
			next.add(key);
		}
		collapsedNodes = next;
	}

	function objectSubtitle(reviewCount: number) {
		if (reviewCount === 0) return "Context only";
		return `${reviewCount} meaningful change${reviewCount === 1 ? "" : "s"}`;
	}

	function childSummary(node: UnitySemanticNode) {
		const ownCount = nodeReviewChanges(node).length;
		const childCount = node.children.length;
		const parts = [];
		if (ownCount > 0) parts.push(`${ownCount} change${ownCount === 1 ? "" : "s"}`);
		if (childCount > 0) parts.push(`${childCount} nested item${childCount === 1 ? "" : "s"}`);
		return parts.join(" • ");
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
				<span class="unity-review-row__label">{item.label}</span>
				{#if showChangeBadge(item.changeKind)}
					<Badge kind="soft" style={changeKindStyle(item.changeKind)}>
						{changeKindLabel(item.changeKind)}
					</Badge>
				{/if}
				{#if item.note}
					<span class="unity-review-row__note text-11">{item.note}</span>
				{/if}
			</div>
			<div class="unity-review-change">
				<span class="unity-review-value old">{item.before}</span>
				<span class="unity-review-arrow">-&gt;</span>
				<span class="unity-review-value new">{item.after}</span>
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
					{#if change.oldReference}
						<code>before asset: {change.oldReference.path}</code>
					{/if}
					{#if change.newValue != null}
						<code>after: {change.newValue}</code>
					{/if}
					{#if change.newReference}
						<code>after asset: {change.newReference.path}</code>
					{/if}
				{/each}
			</details>
		</div>
	</div>
{/snippet}

{#snippet nestedReviewNode(node: UnitySemanticNode, index: number, depth = 0, prefix = "")}
	{@const items = nodeReviewChanges(node, `${prefix}${index}/`)}
	{@const collapseKey = nodeCollapseKey(node, index, prefix)}
	{@const collapsed = isNodeCollapsed(collapseKey)}
	{@const canCollapse = items.length > 0 || node.children.length > 0}
	<div class="unity-child-node" style={`--depth: ${depth}`}>
		<div class="unity-child-node__header">
			<button
				type="button"
				class="unity-collapse-button"
				class:expanded={!collapsed}
				disabled={!canCollapse}
				aria-label={collapsed
					? `Expand ${nodeDisplayName(node)}`
					: `Collapse ${nodeDisplayName(node)}`}
				aria-expanded={!collapsed}
				onclick={() => toggleNodeCollapsed(collapseKey)}
			>
				<Icon name="chevron-right" size={14} />
			</button>
			<div class="unity-child-node__select">
				{@render selectionControl(node.selection, nodeSelectionLabel(node.kind))}
			</div>
			<div class="unity-child-node__title-block">
				<div class="unity-child-node__title">
					<span class="unity-row-kind text-11">{nodeKindLabel(node.kind)}</span>
					<span>{nodeDisplayName(node)}</span>
					{#if showChangeBadge(node.changeKind)}
						<Badge kind="soft" style={changeKindStyle(node.changeKind)}>
							{changeKindLabel(node.changeKind)}
						</Badge>
					{/if}
					{#if childSummary(node)}
						<span class="unity-child-node__meta text-11">{childSummary(node)}</span>
					{/if}
				</div>
				{#if nodePathContext(node)}
					<div class="unity-child-node__path text-11">{nodePathContext(node)}</div>
				{/if}
			</div>
		</div>
		{#if !collapsed}
			{#if items.length > 0}
				<div class="unity-child-node__changes">
					{#each items as item (item.key)}
						{@render reviewChangeRow(item)}
					{/each}
				</div>
			{/if}
			{#if node.children.length > 0}
				<div class="unity-child-node__children">
					{#each node.children as child, childIndex (nodeKey(child, childIndex, `${node.id}:nested:`))}
						{@render nestedReviewNode(child, childIndex, depth + 1, `${prefix}${node.id}:`)}
					{/each}
				</div>
			{/if}
		{/if}
	</div>
{/snippet}

{#snippet objectSection(node: UnitySemanticNode, index: number, prefix = "")}
	{@const items = nodeReviewChanges(node, `${prefix}${index}/`)}
	{@const totalItems = collectReviewChanges(node, `${prefix}${index}/`).length}
	{@const collapseKey = nodeCollapseKey(node, index, prefix)}
	{@const collapsed = isNodeCollapsed(collapseKey)}
	{@const canCollapse = items.length > 0 || node.children.length > 0}
	<section class="unity-object">
		<div class="unity-object__header">
			<button
				type="button"
				class="unity-collapse-button unity-collapse-button--object"
				class:expanded={!collapsed}
				disabled={!canCollapse}
				aria-label={collapsed
					? `Expand ${nodeDisplayName(node)}`
					: `Collapse ${nodeDisplayName(node)}`}
				aria-expanded={!collapsed}
				onclick={() => toggleNodeCollapsed(collapseKey)}
			>
				<Icon name="chevron-right" size={14} />
			</button>
			<div class="unity-object__select">
				{@render selectionControl(node.selection, nodeSelectionLabel(node.kind))}
			</div>
			<div class="unity-object__title-block">
				<div class="unity-object__title">
					<span class="unity-row-kind text-11">{nodeKindLabel(node.kind)}</span>
					<span>{nodeDisplayName(node)}</span>
					{#if showChangeBadge(node.changeKind)}
						<Badge kind="soft" style={changeKindStyle(node.changeKind)}>
							{changeKindLabel(node.changeKind)}
						</Badge>
					{/if}
				</div>
				<div class="unity-object__meta text-11">
					<span>{objectSubtitle(totalItems)}</span>
					{#if nodePathContext(node)}
						<span>{nodePathContext(node)}</span>
					{/if}
				</div>
			</div>
		</div>

		{#if !collapsed}
			{#if items.length > 0}
				<div class="unity-review-list">
					{#each items as item (item.key)}
						{@render reviewChangeRow(item)}
					{/each}
				</div>
			{/if}
			{#if node.children.length > 0}
				<div class="unity-child-list">
					{#each node.children as child, childIndex (nodeKey(child, childIndex, `${node.id}:children:`))}
						{@render nestedReviewNode(child, childIndex, 0, `${prefix}${index}/`)}
					{/each}
				</div>
			{/if}
		{/if}
	</section>
{/snippet}

{#snippet semanticContent(diff: UnitySemanticDiff | null)}
	{@const tooLong = tooLongWarning(diff)}
	{#if tooLong}
		<div class="unity-warning unity-warning--too-large">
			<p class="text-13 text-semibold">Semantic diff is too large</p>
			<p class="text-12 clr-text-2">{tooLong.message}</p>
			{#if onShowRaw}
				<Button kind="outline" size="tag" onclick={onShowRaw}>Show raw diff</Button>
			{/if}
		</div>
	{:else if !diff || diff.nodes.length === 0}
		<div class="unity-empty">
			<p class="text-13 text-semibold">Semantic view is not available for this change.</p>
			<p class="text-12 clr-text-2">Raw diff is still available.</p>
			{#if onShowRaw}
				<Button kind="outline" size="tag" onclick={onShowRaw}>Show raw diff</Button>
			{/if}
		</div>
	{:else}
		<div class="unity-summary">
			<div>
				<p class="text-13 text-semibold">{fileKindLabel(diff.fileKind)} review</p>
				<p class="text-12 clr-text-2">
					Grouped by object and component. Open serialized values only when you need raw scene
					values.
				</p>
			</div>
			<div class="unity-summary__badges">
				<Badge kind="soft" style="gray">{summaryText(diff)}</Badge>
			</div>
		</div>
		{#if diff.warnings.length > 0}
			<div class="unity-warning">
				<p class="text-12 text-semibold">Semantic view is partial</p>
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
					<p class="text-12 clr-text-2">Loading objects...</p>
				</div>
			{/if}
		</div>
	{/if}
{/snippet}

{#if !requested}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Semantic view is paused.</p>
		<p class="text-12 clr-text-2">Load it when you need object-level review.</p>
		<Button kind="outline" size="tag" onclick={() => loadSemanticDiff()}>Load semantic view</Button>
		{#if onShowRaw}
			<Button kind="ghost" size="tag" onclick={onShowRaw}>Show raw diff</Button>
		{/if}
	</div>
{:else if loading}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Scene is loading...</p>
		{#if onShowRaw}
			<Button kind="ghost" size="tag" onclick={onShowRaw}>Show raw diff</Button>
		{/if}
	</div>
{:else if error}
	<div class="unity-empty">
		<p class="text-13 text-semibold">Semantic view failed to load.</p>
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
		padding: 9px 12px;
		gap: 8px;
		background-color: var(--bg-1);
	}

	.unity-collapse-button {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		justify-content: center;
		width: 18px;
		height: 18px;
		margin-top: 1px;
		border-radius: var(--radius-s);
		color: var(--text-3);
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast),
			transform var(--transition-medium);

		&:hover:not(:disabled) {
			background-color: var(--bg-3);
			color: var(--text-1);
		}

		&:disabled {
			visibility: hidden;
		}
	}

	.unity-collapse-button.expanded {
		transform: rotate(90deg);
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

	.unity-child-list,
	.unity-child-node__children {
		display: flex;
		flex-direction: column;
	}

	.unity-child-list {
		padding: 6px 0 8px 28px;
		box-shadow: inset 0 1px 0 var(--border-3);
	}

	.unity-child-node {
		position: relative;
		margin-left: calc(var(--depth) * 14px);
		padding: 2px 0;
	}

	.unity-child-node__header {
		display: flex;
		align-items: flex-start;
		min-width: 0;
		padding: 7px 12px 7px 8px;
		gap: 8px;
	}

	.unity-child-node__select {
		flex-shrink: 0;
		width: 16px;
	}

	.unity-child-node__title-block {
		flex: 1;
		min-width: 0;
	}

	.unity-child-node__title {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--text-2);
		font-weight: 500;
	}

	.unity-child-node__meta {
		color: var(--text-3);
		font-weight: 500;
	}

	.unity-child-node__path {
		margin-top: 3px;
		overflow: hidden;
		color: var(--text-3);
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.unity-child-node__changes {
		display: flex;
		flex-direction: column;
		margin: 2px 0 6px 42px;
		border-top: 1px solid var(--border-3);
	}

	.unity-review-list {
		display: flex;
		flex-direction: column;
		padding-left: 42px;
	}

	.unity-review-row {
		display: flex;
		padding: 8px 12px 8px 0;
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
		color: var(--text-2);
		font-weight: 600;
	}

	.unity-review-row__label {
		font-size: 13px;
	}

	.unity-review-row__note,
	.unity-details {
		color: var(--text-3);
	}

	.unity-review-change {
		display: flex;
		align-items: center;
		min-width: 0;
		margin-top: 5px;
		gap: 6px;
	}

	.unity-review-value {
		display: inline-block;
		min-width: 0;
		padding: 1px 4px;
		overflow: hidden;
		border-radius: var(--radius-s);
		font-weight: 500;
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.unity-review-value.old {
		background-color: color-mix(in srgb, var(--diff-deletion-line-bg) 70%, transparent);
		color: var(--diff-deletion-count-text);
	}

	.unity-review-value.new {
		background-color: color-mix(in srgb, var(--diff-addition-line-bg) 70%, transparent);
		color: var(--diff-addition-count-text);
	}

	.unity-review-arrow {
		flex: 0 0 auto;
		color: var(--text-3);
		font-size: 12px;
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
