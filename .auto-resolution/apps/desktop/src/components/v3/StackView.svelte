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
	<BranchList {projectId} {stackId} />
</ConfigurableScrollableContainer>
