<script lang="ts">
	import BaseBranchSwitch from '$components/BaseBranchSwitch.svelte';
	import DetailsForm from '$components/DetailsForm.svelte';
	import ForgeForm from '$components/ForgeForm.svelte';
	import GerritForm from '$components/GerritForm.svelte';
	import RemoveProjectForm from '$components/RemoveProjectForm.svelte';

	const { projectId }: { projectId: string } = $props();
</script>

<DetailsForm {projectId} />
<BaseBranchSwitch {projectId} />
<GerritForm {projectId} />
<ForgeForm {projectId} />
<RemoveProjectForm {projectId} />
