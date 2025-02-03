<script lang="ts">
	import type { WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		branch: WorkspaceBranch;
	}

	const { branch }: Props = $props();
</script>

<div class="branch-container"></div>

<style>
	.branch-container {
		display: flex;
		flex-direction: column;
	}
</style>
