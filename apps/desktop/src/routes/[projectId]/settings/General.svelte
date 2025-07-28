<script lang="ts">
	import RemoveProjectForm from '$components/projectManagement/RemoveProjectForm.svelte';
	import DetailsForm from '$components/shared/DetailsForm.svelte';
	import ForgeForm from '$components/shared/ForgeForm.svelte';
	import BaseBranchSwitch from '$components/shared/branches/BaseBranchSwitch.svelte';

	const { projectId }: { projectId: string } = $props();
</script>

<DetailsForm {projectId} />
<BaseBranchSwitch {projectId} />
<ForgeForm {projectId} />
<RemoveProjectForm {projectId} />
