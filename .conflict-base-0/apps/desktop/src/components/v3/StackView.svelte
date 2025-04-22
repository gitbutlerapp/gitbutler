<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import BranchList from '$components/v3/BranchList.svelte';

	interface Props {
		projectId: string;
		stackId: string;
	}

	const { stackId, projectId }: Props = $props();
</script>

<ConfigurableScrollableContainer wide>
	<div class="inner">
		<BranchList {projectId} {stackId} />
	</div>
</ConfigurableScrollableContainer>

<style>
	.inner {
		padding: 14px;
	}
</style>
