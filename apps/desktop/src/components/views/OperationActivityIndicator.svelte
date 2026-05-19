<script lang="ts">
	import { BACKEND } from "$lib/backend";
	import {
		operationActivityStore,
		recordGitOperationProgress,
		type GitOperationProgress,
	} from "$lib/activity/operationActivity";
	import { inject } from "@gitbutler/core/context";
	import { Icon, Tooltip } from "@gitbutler/ui";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const backend = inject(BACKEND);
	const activeActivity = $derived($operationActivityStore.activities[0]);
	let elapsedTick = $state(Date.now());
	let contentElement = $state<HTMLDivElement>();
	let contentWidth = $state(0);

	$effect(() => {
		const unlisten = backend.listen<GitOperationProgress>(
			`project://${projectId}/git_operation_progress`,
			({ payload }) => recordGitOperationProgress(projectId, payload),
		);
		return () => {
			void unlisten();
		};
	});

	$effect(() => {
		if (!activeActivity) return;
		const timer = window.setInterval(() => {
			elapsedTick = Date.now();
		}, 1000);
		return () => window.clearInterval(timer);
	});

	$effect(() => {
		if (!contentElement) return;
		const updateWidth = () => {
			contentWidth = contentElement?.offsetWidth ?? 0;
		};
		updateWidth();

		const resizeObserver = new ResizeObserver(updateWidth);
		resizeObserver.observe(contentElement);
		return () => resizeObserver.disconnect();
	});

	const elapsedLabel = $derived(
		formatElapsed(activeActivity ? elapsedTick - activeActivity.startedAt : 0),
	);
	const label = $derived(activeActivity ? `${activeActivity.label} ${elapsedLabel}` : "");
	const tooltip = $derived.by(() => {
		if (!activeActivity) return "";
		if (activeActivity.detail) return `${label}. ${activeActivity.detail}`;
		return label;
	});

	function formatElapsed(ms: number): string {
		const seconds = Math.max(0, Math.floor(ms / 1000));
		if (seconds < 60) return `${seconds}s`;
		const minutes = Math.floor(seconds / 60);
		const remainingSeconds = seconds % 60;
		return `${minutes}m ${remainingSeconds}s`;
	}
</script>

<div
	class="operation-activity-frame"
	class:active={!!activeActivity}
	style:width={activeActivity ? `${contentWidth}px` : "0px"}
>
	{#if activeActivity}
		<Tooltip text={tooltip}>
			<div bind:this={contentElement} class="operation-activity" role="status" aria-live="polite">
				<Icon name="spinner" />
				<span class="text-12 text-semibold truncate">{label}</span>
			</div>
		</Tooltip>
	{/if}
</div>

<style>
	.operation-activity-frame {
		--activity-resize-transition: 120ms var(--motion-ease-standard);

		flex: 0 0 auto;
		overflow: hidden;
		opacity: 0;
		transition:
			width var(--activity-resize-transition),
			opacity var(--transition-fast);
	}

	.operation-activity-frame.active {
		opacity: 1;
	}

	.operation-activity {
		display: flex;
		align-items: center;
		width: max-content;
		max-width: 260px;
		height: 28px;
		padding: 0 8px;
		overflow: hidden;
		gap: 6px;
		border: 1px solid var(--border-2);
		border-radius: 100px;
		background: var(--bg-1);
		color: var(--text-2);
	}
</style>
