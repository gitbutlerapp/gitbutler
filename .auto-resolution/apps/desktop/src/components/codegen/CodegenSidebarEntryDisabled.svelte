<script lang="ts">
	import { Icon } from '@gitbutler/ui';

	type Props = {
		branchName: string;
	};

	const { branchName }: Props = $props();
</script>

<div class="greyed-out-branch">
	<div class="branch-header">
		<div class="branch-info">
			<Icon name="branch-remote" opacity={0.3} />
			<p class="text-13 text-semibold truncate full-width opacity-50">{branchName}</p>
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
</style>
