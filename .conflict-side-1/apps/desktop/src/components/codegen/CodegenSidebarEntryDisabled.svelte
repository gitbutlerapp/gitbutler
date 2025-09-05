<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		branchIcon?: Snippet;
	};

	const { branchName, branchIcon }: Props = $props();
</script>

<div class="greyed-out-branch">
	<div class="branch-header">
		<div class="branch-info">
			<div class="icon-wrapper">
				{#if branchIcon}
					{@render branchIcon()}
				{:else}
					<!-- Default fallback icon -->
					<div class="default-icon"></div>
				{/if}
			</div>
			<p class="text-14 text-semibold truncate full-width opacity-50">{branchName}</p>
		</div>
	</div>
</div>

<style lang="postcss">
	.greyed-out-branch {
		flex-shrink: 0;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		opacity: 0.5;
		pointer-events: none;
	}

	.branch-header {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		padding: 12px;
		overflow: hidden;
		gap: 10px;
		background-color: var(--clr-bg-2);
		text-align: left;
	}

	.branch-info {
		display: flex;
		align-items: center;
		width: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.default-icon {
		width: 20px;
		height: 20px;
		border-radius: 4px;
		background-color: var(--clr-text-3);
		opacity: 0.3;
	}

	.icon-wrapper {
		opacity: 0.3;
	}
</style>
