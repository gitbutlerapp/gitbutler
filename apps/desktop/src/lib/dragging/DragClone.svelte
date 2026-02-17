<script lang="ts">
	import BranchHeaderIcon from "$components/BranchHeaderIcon.svelte";
	import { getColorFromCommitState } from "$components/lib";
	import { type CommitStatusType } from "$lib/commits/commit";
	import { type PushStatus } from "$lib/stacks/stack";
	import { Icon, FileIcon } from "@gitbutler/ui";
	import { type DragStateService } from "@gitbutler/ui/drag/dragStateService.svelte";
	import { readable } from "svelte/store";

	type Props = {
		type: "branch" | "commit" | "file" | "folder" | "hunk" | "ai-session";
		label?: string;
		filePath?: string;
		commitType?: CommitStatusType;
		childrenAmount?: number;
		pushStatus?: PushStatus;
		dragStateService?: DragStateService;
	};

	let {
		type,
		label,
		filePath,
		commitType,
		childrenAmount = 1,
		pushStatus,
		dragStateService,
	}: Props = $props();

	const fallbackDropLabelStore = readable<string | undefined>(undefined);
	const dropLabel = $derived(dragStateService?.dropLabel ?? fallbackDropLabelStore);

	const commitColor = $derived(
		type === "commit" && commitType ? getColorFromCommitState(commitType, false) : undefined,
	);
</script>

{#snippet dropLabelSnippet(opt: { label?: string; amount?: number })}
	<div class="drag-action-label-container">
		{#if opt.label}
			<div class="text-11 text-bold drag-action-label drag-action-label__action">
				<span>{opt.label}</span>

				<div class="drag-action-label-icon">
					<Icon name="arrow-down" size={12} />
				</div>
			</div>
		{/if}
		{#if opt.amount && opt.amount > 1}
			<div class="text-11 text-bold drag-action-label" class:has-label={opt.label}>
				{opt.amount}
			</div>
		{/if}
	</div>
{/snippet}

{#if type === "branch"}
	<div class="draggable-branch-card">
		<div class="drag-animation-wrapper" class:activated={$dropLabel !== undefined}>
			{@render dropLabelSnippet({ label: $dropLabel, amount: childrenAmount })}
			{#if pushStatus}
				<BranchHeaderIcon iconName="branch-local" color="var(--clr-commit-local)" small />
			{/if}
			<span class="truncate text-15 text-bold">
				{label}
			</span>
		</div>
	</div>
{:else if type === "commit"}
	<div
		class="draggable-commit"
		class:draggable-commit-local={commitType === "LocalOnly" ||
			commitType === "Integrated" ||
			commitType === "Base"}
		class:draggable-commit-remote={commitType !== "LocalOnly" &&
			commitType !== "Integrated" &&
			commitType !== "Base"}
		style:--commit-color={commitColor}
	>
		<div class="drag-animation-wrapper" class:activated={$dropLabel !== undefined}>
			{@render dropLabelSnippet({ label: $dropLabel, amount: childrenAmount })}
			<div class="draggable-commit-indicator"></div>
			<div class="truncate text-13 text-semibold draggable-commit-label">
				{label || "Empty commit"}
			</div>
		</div>
	</div>
{:else if type === "ai-session"}
	<div class="dragchip-container">
		<div class="drag-animation-wrapper" class:activated={$dropLabel !== undefined}>
			{@render dropLabelSnippet({ label: $dropLabel, amount: childrenAmount })}
			<div class="dragchip-ai-session-container">
				<Icon name="ai-small" />
				{#if label}
					<span class="text-12 text-semibold truncate dragchip-ai-session-label">{label}</span>
				{/if}
				<Icon name="draggable" />
			</div>
		</div>
	</div>
{:else}
	<!-- File, Folder, or Hunk chips -->
	<div class="dragchip-container">
		<div
			class="drag-animation-wrapper"
			class:activated={$dropLabel !== undefined}
			class:dragchip-two={childrenAmount === 2}
			class:dragchip-multiple={childrenAmount > 2}
		>
			{@render dropLabelSnippet({ label: $dropLabel, amount: childrenAmount })}
			<div class="dragchip">
				{#if type === "file"}
					<div class="dragchip-file-container">
						<FileIcon fileName={filePath || ""} />
						<span class="text-12 text-semibold truncate dragchip-file-name">
							{label || "Empty file"}
						</span>
					</div>
				{:else if type === "folder"}
					<div class="dragchip-file-container">
						<FileIcon fileName="folder-close" color="var(--clr-text-2)" />
						<span class="text-12 text-semibold truncate dragchip-file-name">
							{label || "Empty folder"}
						</span>
					</div>
				{:else if type === "hunk"}
					<div class="dragchip-hunk-container">
						<div class="dragchip-hunk-decorator">〈/〉</div>
						<span class="dragchip-hunk-label">{label || "Empty hunk"}</span>
					</div>
				{/if}
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	/* DRAG CHIPS.
	 * General styles
	 * Basic container for single and multiple items */
	.dragchip-container {
		display: flex;
		pointer-events: none;
	}

	.drag-animation-wrapper {
		display: flex;
		align-items: center;

		&.activated {
			animation: dragchip-scale 0.23s ease forwards;
		}
	}

	@keyframes dragchip-scale {
		0% {
			transform: scale(1);
		}
		50% {
			transform: scale(1.08);
		}
		100% {
			transform: scale(1);
		}
	}

	/* root */
	:global(:root) {
		--chip-shadow: 0 12px 30px rgba(0, 0, 0, 0.2);
	}
	/* dark mode */
	:global(:root.dark) {
		--chip-shadow: 0 12px 30px rgba(0, 0, 0, 0.3);
	}

	.drag-action-label-container {
		display: flex;
		position: absolute;
		top: -12px;
		right: -12px;
		gap: 2px;
		pointer-events: none;
	}

	.drag-action-label {
		display: flex;
		z-index: 4;
		align-items: center;
		padding: 4px 7px;
		border-radius: 100px;
		background-color: var(--clr-theme-gray-element);
		color: var(--clr-theme-gray-on-element);
		white-space: nowrap;
		pointer-events: none;
	}

	.drag-action-label__action {
		gap: 4px;
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}

	.drag-action-label-icon {
		display: flex;
		animation: icon-shifting 1s ease infinite;
	}

	@keyframes icon-shifting {
		0% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-2px);
		}
		100% {
			transform: translateY(0);
		}
	}

	.dragchip {
		display: flex;
		z-index: 3;
		position: relative;
		min-width: 50px;
		max-width: 240px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		box-shadow: var(--chip-shadow);
	}

	/* if dragging more then one item */
	.drag-animation-wrapper.dragchip-two::after,
	.drag-animation-wrapper.dragchip-multiple::before,
	.drag-animation-wrapper.dragchip-multiple::after {
		position: absolute;
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		content: "";
	}

	.drag-animation-wrapper.dragchip-two {
		&::after {
			z-index: 2;
			top: 6px;
			left: 6px;
		}
	}

	.drag-animation-wrapper.dragchip-multiple {
		&::before {
			z-index: 2;
			top: 6px;
			left: 6px;
		}

		&::after {
			z-index: 1;
			top: 12px;
			left: 12px;
		}
	}

	/* FILE DRAG */
	.dragchip-file-container {
		display: flex;
		position: relative;
		align-items: center;
		padding: 8px;
		overflow: hidden;
		gap: 6px;
	}

	.dragchip-file-name {
		color: var(--clr-text-1);
	}

	/* HUNK DRAG */
	.dragchip-hunk-container {
		display: flex;
		font-size: 12px;
		font-family: var(--font-mono);
	}

	.dragchip-hunk-decorator {
		padding: 6px 5px;
		border-right: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) 0 0 var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		font-variant-ligatures: none;
		letter-spacing: -1px;
	}

	.dragchip-hunk-label {
		padding: 6px 7px;
	}

	/* AI SESSION DRAG */
	.dragchip-container {
		& .drag-animation-wrapper:has(.dragchip-ai-session-container) {
			width: auto;
		}
	}

	.dragchip-ai-session-container {
		display: flex;
		align-items: center;
		height: var(--size-tag);
		padding: 0 4px;
		gap: 4px;
		border: none;
		border-radius: var(--radius-m);
		background-position: 0% 50%;
		background-size: 200% 200%;
		background: var(--codegen-gradient);
		box-shadow: var(--chip-shadow);
		color: var(--codegen-color);
	}

	/* BRANCH DRAG CARD */
	.draggable-branch-card {
		pointer-events: none;

		& .drag-animation-wrapper {
			min-width: 50px;
			max-width: 220px;
			height: 36px;
			padding: 0 10px;
			gap: 10px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-1);
			box-shadow: var(--chip-shadow);
		}
	}

	/* COMMIT DRAG CARD */
	.draggable-commit {
		pointer-events: none;

		& .drag-animation-wrapper {
			position: relative;
			min-width: 50px;
			max-width: 240px;
			height: 36px;
			padding: 0 10px;
			gap: 10px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-1);
			box-shadow: var(--chip-shadow);

			&::before {
				z-index: 1;
				position: absolute;
				top: 0;
				left: 14px;
				width: 2px;
				height: 100%;
				background-color: var(--commit-color);
				content: "";
			}
		}
	}

	.draggable-commit-indicator {
		z-index: 2;
		flex-shrink: 0;
		width: 10px;
		height: 10px;
		outline: 3px solid var(--clr-bg-1);
		background-color: var(--commit-color);
	}

	.draggable-commit-local {
		& .draggable-commit-indicator {
			border-radius: 50%;
		}
	}

	.draggable-commit-remote {
		& .draggable-commit-indicator {
			transform: rotate(45deg) scale(0.9);
			border-radius: 2px;
		}
	}

	/* Dim the original element when it's being dragged */
	:global(.dragging) {
		opacity: 0.5;
		pointer-events: none;
	}
</style>
