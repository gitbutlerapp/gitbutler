<script lang="ts">
	import { Icon } from "@gitbutler/ui";
	import type { ConflictHunk } from "@gitbutler/but-sdk";

	type Props = {
		hunk: ConflictHunk;
		index: number;
		total: number;
	};

	const { hunk, index, total }: Props = $props();
</script>

<div class="conflict-card">
	<div class="conflict-card__title">
		<Icon name="warning" color="var(--fill-warn-bg)" />
		<span class="text-12 text-semibold">
			Unresolved conflict{total > 1 ? ` ${index + 1} of ${total}` : ""}
		</span>
	</div>
	<div class="conflict-card__section ours">
		<div class="text-11 text-semibold conflict-card__label">Current base</div>
		<pre class="text-12 conflict-card__content">{hunk.ours}</pre>
	</div>
	{#if hunk.base !== null}
		<div class="conflict-card__section">
			<div class="text-11 text-semibold conflict-card__label">Common ancestor</div>
			<pre class="text-12 conflict-card__content">{hunk.base}</pre>
		</div>
	{/if}
	<div class="conflict-card__section theirs">
		<div class="text-11 text-semibold conflict-card__label">This commit</div>
		<pre class="text-12 conflict-card__content">{hunk.theirs}</pre>
	</div>
</div>

<style>
	.conflict-card {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
	}

	.conflict-card__title {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		gap: 6px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-warn);
		color: var(--text-warn);
	}

	.conflict-card__section {
		display: flex;
		flex-direction: column;

		&:not(:last-child) {
			border-bottom: 1px solid var(--border-2);
		}

		&.ours .conflict-card__content {
			background-color: var(--diff-deletion-line-bg);
		}

		&.theirs .conflict-card__content {
			background-color: var(--diff-addition-line-bg);
		}
	}

	.conflict-card__label {
		padding: 4px 10px;
		background-color: var(--bg-2);
		color: var(--text-3);
		letter-spacing: 0.04em;
		text-transform: uppercase;
	}

	.conflict-card__content {
		margin: 0;
		padding: 6px 10px;
		overflow-x: auto;
		font-size: var(--diff-font-size, 12px);
		font-family: var(--font-mono);
		white-space: pre;
	}
</style>
