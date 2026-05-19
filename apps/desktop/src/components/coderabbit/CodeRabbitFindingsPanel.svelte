<script lang="ts">
	import { CODERABBIT_SERVICE } from "$lib/coderabbit/coderabbit";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { inject } from "@gitbutler/core/context";
	import { Button } from "@gitbutler/ui";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const codeRabbitService = inject(CODERABBIT_SERVICE);
	const clipboard = inject(CLIPBOARD_SERVICE);
	const findingsQuery = $derived(codeRabbitService.findings(projectId));
	const updateFinding = codeRabbitService.updateFinding;
	const findings = $derived(
		(findingsQuery.response ?? []).filter((finding) => finding.status === "open"),
	);
	let expanded = $state(false);

	function findingText(finding: (typeof findings)[number]) {
		return [finding.title, finding.body, finding.suggestedPatch].filter(Boolean).join("\n\n");
	}
</script>

{#if findings.length > 0}
	<div class="coderabbit-panel" class:expanded>
		<div class="coderabbit-panel__header">
			<button type="button" onclick={() => (expanded = !expanded)}>
				CodeRabbit recommendations ({findings.length})
			</button>
			<Button
				kind="ghost"
				size="tag"
				icon={expanded ? "chevron-down" : "chevron-up"}
				onclick={() => (expanded = !expanded)}
			/>
		</div>
		{#if expanded}
			<div class="coderabbit-panel__body">
				{#each findings as finding (finding.id)}
					<div class="finding">
						<div class="finding__meta">
							<span>{finding.severity}</span>
							<span>{finding.path}{finding.newLine ? `:${finding.newLine}` : ""}</span>
						</div>
						<div class="finding__title">{finding.title}</div>
						{#if finding.body}
							<p>{finding.body}</p>
						{/if}
						<div class="finding__actions">
							<Button
								kind="ghost"
								size="tag"
								icon="copy"
								onclick={() =>
									clipboard.write(findingText(finding), { message: "Recommendation copied" })}
							>
								Copy
							</Button>
							<Button
								kind="ghost"
								size="tag"
								icon="cross"
								onclick={() =>
									updateFinding({
										projectId,
										update: { findingId: finding.id, status: "dismissed" },
									})}
							>
								Dismiss
							</Button>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.coderabbit-panel {
		display: flex;
		z-index: var(--z-popover);
		position: fixed;
		right: 18px;
		bottom: 18px;
		flex-direction: column;
		width: min(420px, calc(100vw - 36px));
		max-height: min(520px, calc(100vh - 36px));
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
		box-shadow: var(--fx-shadow-m);
	}

	.coderabbit-panel__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 10px;
		border-bottom: 1px solid var(--border-2);

		button:first-child {
			color: var(--text-1);
			font-weight: 600;
			font-size: 12px;
		}
	}

	.coderabbit-panel__body {
		display: flex;
		flex-direction: column;
		padding: 8px;
		overflow: auto;
		gap: 8px;
	}

	.finding {
		padding: 8px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-0);
	}

	.finding__meta {
		display: flex;
		justify-content: space-between;
		gap: 8px;
		color: var(--text-3);
		font-size: 11px;
		text-transform: capitalize;
	}

	.finding__title {
		margin-top: 4px;
		color: var(--text-1);
		font-weight: 600;
		font-size: 12px;
	}

	p {
		margin: 6px 0 0;
		color: var(--text-2);
		font-size: 12px;
		line-height: 1.35;
		white-space: pre-wrap;
	}

	.finding__actions {
		display: flex;
		margin-top: 8px;
		gap: 6px;
	}
</style>
