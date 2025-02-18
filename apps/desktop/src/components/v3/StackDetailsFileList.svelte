<script lang="ts">
	import { Commit } from '$lib/commits/commit';

	interface Props {
		commit?: Commit;
	}

	const { commit }: Props = $props();
</script>

<div class="wrapper">
	<div class="header text-13 text-bold">Changed files</div>
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		padding: 14px 14px 16px 14px;
	}
</style>
