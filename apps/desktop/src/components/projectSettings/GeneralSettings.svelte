<script lang="ts">
	import BaseBranchSwitch from "$components/forge/BaseBranchSwitch.svelte";
	import DetailsForm from "$components/projectSettings/DetailsForm.svelte";
	import ForgeForm from "$components/projectSettings/ForgeForm.svelte";
	import GerritForm from "$components/projectSettings/GerritForm.svelte";
	import RemoveProjectForm from "$components/projectSettings/RemoveProjectForm.svelte";
	import { Spacer } from "@gitbutler/ui";

	const { projectId }: { projectId: string } = $props();
</script>

<DetailsForm {projectId} />
<BaseBranchSwitch {projectId} />
<GerritForm {projectId} />
<ForgeForm {projectId} />
<Spacer />
<RemoveProjectForm {projectId} />
