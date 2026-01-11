<script lang="ts">
	import BaseBranchSwitch from '$components/BaseBranchSwitch.svelte';
	import DetailsForm from '$components/DetailsForm.svelte';
	import ForgeForm from '$components/ForgeForm.svelte';
	import GerritForm from '$components/GerritForm.svelte';
	import RemoveProjectForm from '$components/RemoveProjectForm.svelte';
	import { projectDisableCodegen } from '$lib/config/config';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;

	const codegenDisabled = $derived(projectDisableCodegen(projectId));
</script>

<DetailsForm {projectId} />
<BaseBranchSwitch {projectId} />
<GerritForm {projectId} />
<ForgeForm {projectId} />
<!-- Maybe we could inline more settings here -->
<CardGroup.Item standalone labelFor="disable-codegen">
	{#snippet title()}
		{$t('settings.project.disableCodegen.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.project.disableCodegen.caption')}
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
