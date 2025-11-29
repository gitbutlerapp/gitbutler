<script lang="ts">
	import BaseBranchSwitch from '$components/BaseBranchSwitch.svelte';
	import DetailsForm from '$components/DetailsForm.svelte';
	import ForgeForm from '$components/ForgeForm.svelte';
	import GerritForm from '$components/GerritForm.svelte';
	import RemoveProjectForm from '$components/RemoveProjectForm.svelte';
	import { projectDisableCodegen } from '$lib/config/config';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const codegenDisabled = $derived(projectDisableCodegen(projectId));
</script>

<DetailsForm {projectId} />
<BaseBranchSwitch {projectId} />
<GerritForm {projectId} />
<ForgeForm {projectId} />
<!-- Maybe we could inline more settings here -->
<CardGroup.Item labelFor="disable-codegen">
	{#snippet title()}
		Disable codegen
	{/snippet}
	{#snippet caption()}
		Hides the codegen button in the branch headers.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="disable-codegen"
			checked={$codegenDisabled}
			onclick={() => ($codegenDisabled = !$codegenDisabled)}
		/>
	{/snippet}
</CardGroup.Item>
<Spacer />
<RemoveProjectForm {projectId} />
