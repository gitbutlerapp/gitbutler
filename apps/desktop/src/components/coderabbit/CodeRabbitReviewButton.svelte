<script lang="ts">
	import { showError } from "$lib/error/showError";
	import { CODERABBIT_SERVICE } from "$lib/coderabbit/coderabbit";
	import { inject } from "@gitbutler/core/context";
	import { Button } from "@gitbutler/ui";
	import type { CodeRabbitWorkflowId } from "$lib/coderabbit/coderabbit";

	type Props = {
		projectId: string;
		files?: string[];
		compact?: boolean;
	};

	const { projectId, files = [], compact = false }: Props = $props();

	const codeRabbitService = inject(CODERABBIT_SERVICE);
	const statusQuery = $derived(codeRabbitService.status(projectId));
	const findingsQuery = $derived(codeRabbitService.findings(projectId));
	const status = $derived(statusQuery.response);
	const findings = $derived(
		(findingsQuery.response ?? []).filter((finding) => finding.status === "open"),
	);
	const review = codeRabbitService.review;
	const login = codeRabbitService.login;
	const cancel = codeRabbitService.cancel;
	const writeDefaultConfig = codeRabbitService.writeDefaultConfig;
	let workflowMenuOpen = $state(false);
	let activeWorkflow = $state<CodeRabbitWorkflowId>("default");
	let activeReviewId = $state<string | undefined>();
	let reviewing = $state(false);
	let cancelling = $state(false);

	const isReviewing = $derived(reviewing || !!activeReviewId || !!status?.activeReviewId);
	const buttonLabel = $derived.by(() => {
		if (isReviewing) return "Reviewing...";
		if (!status?.cliAvailable) return "CodeRabbit unavailable";
		if (!status.authenticated) return "Sign in to CodeRabbit";
		if (findings.length > 0) return `CodeRabbit (${findings.length})`;
		return compact ? "CodeRabbit" : "Review with CodeRabbit";
	});

	async function runReview(workflows: CodeRabbitWorkflowId[] = ["default"]) {
		activeWorkflow = workflows[0] ?? "default";
		if (!status?.cliAvailable) {
			showError("CodeRabbit CLI unavailable", status?.error ?? "Install the CodeRabbit CLI first.");
			return;
		}
		if (!status.authenticated) {
			await login({ projectId });
			return;
		}
		try {
			reviewing = true;
			cancelling = false;
			const reviewId = newReviewId();
			activeReviewId = reviewId;
			await review({
				projectId,
				request: {
					reviewId,
					reviewType: "uncommitted",
					files,
					workflows,
				},
			});
		} catch (error) {
			if (!cancelling) {
				showError("CodeRabbit review failed", error);
			}
		} finally {
			reviewing = false;
			cancelling = false;
			activeReviewId = undefined;
		}
	}

	async function cancelReview() {
		const reviewId = activeReviewId ?? status?.activeReviewId;
		if (!reviewId) return;
		try {
			cancelling = true;
			await cancel({ projectId, reviewId });
		} catch (error) {
			showError("Failed to cancel CodeRabbit review", error);
		}
	}

	function newReviewId() {
		return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
	}

	async function createConfig() {
		try {
			await writeDefaultConfig({ projectId });
			workflowMenuOpen = false;
		} catch (error) {
			showError("Failed to create CodeRabbit config", error);
		}
	}
</script>

<div class="coderabbit-review">
	<Button
		type="button"
		kind="outline"
		size="tag"
		icon={isReviewing ? "spinner" : "robot"}
		disabled={isReviewing || statusQuery.result.isLoading}
		tooltip={status?.username ? `Signed in as ${status.username}` : status?.error}
		onclick={() => runReview(["default"])}
	>
		{buttonLabel}
	</Button>
	<Button
		type="button"
		kind="ghost"
		size="tag"
		icon={isReviewing ? "cross" : "chevron-down"}
		tooltip={isReviewing ? "Cancel CodeRabbit review" : "CodeRabbit review workflows"}
		onclick={() => (isReviewing ? cancelReview() : (workflowMenuOpen = !workflowMenuOpen))}
	/>

	{#if workflowMenuOpen}
		<div class="workflow-menu">
			<button
				type="button"
				class:active={activeWorkflow === "default"}
				onclick={() => runReview(["default"])}
			>
				Default review
			</button>
			<button
				type="button"
				class:active={activeWorkflow === "performance"}
				onclick={() => runReview(["performance"])}
			>
				Performance review
			</button>
			<button
				type="button"
				class:active={activeWorkflow === "security"}
				onclick={() => runReview(["security"])}
			>
				Security review
			</button>
			<button
				type="button"
				class:active={activeWorkflow === "correctness"}
				onclick={() => runReview(["correctness"])}
			>
				Correctness review
			</button>
			{#if status?.cliAvailable && !status.configExists}
				<button type="button" onclick={createConfig}>Create .coderabbit.yaml</button>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.coderabbit-review {
		display: flex;
		position: relative;
		align-items: center;
		gap: 4px;
	}

	.workflow-menu {
		display: flex;
		z-index: var(--z-popover);
		position: absolute;
		right: 0;
		bottom: calc(100% + 6px);
		flex-direction: column;
		width: 190px;
		padding: 4px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
		box-shadow: var(--fx-shadow-m);

		button {
			padding: 7px 8px;
			border-radius: var(--radius-s);
			color: var(--text-1);
			font-size: 12px;
			text-align: left;

			&:hover,
			&.active {
				background-color: var(--bg-2);
			}
		}
	}
</style>
