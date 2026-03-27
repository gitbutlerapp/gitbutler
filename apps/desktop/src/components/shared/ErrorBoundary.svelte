<script lang="ts">
	import { logError } from "$lib/error/logError";
	import { Icon } from "@gitbutler/ui";
	import type { Snippet } from "svelte";

	type Props = {
		children: Snippet;
		title?: string;
		compact?: boolean;
	};

	const { children, title = "Something went wrong", compact = false }: Props = $props();
</script>

<svelte:boundary onerror={(e) => logError(e, { skipToast: true })}>
	{@render children()}

	{#snippet failed(error: unknown)}
		{#if compact}
			<div class="boundary-error compact">
				<Icon name="warning" />
				<span class="text-12 truncate boundary-error__title">{title}</span>
			</div>
		{:else}
			<div class="boundary-error">
				<p class="text-13 boundary-error__title">{title}</p>
				{#if error instanceof Error && error.message}
					<p class="text-12 boundary-error__message">{error.message}</p>
				{/if}
			</div>
		{/if}
	{/snippet}
</svelte:boundary>

<style lang="postcss">
	.boundary-error {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		padding: 16px;
		gap: 8px;
	}

	.boundary-error.compact {
		flex-direction: row;
		align-items: center;
		height: 44px;
		padding: 0 12px;
		padding-left: 14px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
	}

	.boundary-error__title {
		color: var(--text-1);
	}

	.compact .boundary-error__title {
		flex: 1;
		color: var(--text-2);
	}

	.boundary-error__message {
		color: var(--text-2);
	}
</style>
