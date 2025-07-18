<script lang="ts">
	import Section from '$components/Section.svelte';
	import { Project } from '$lib/project/project';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let omitCertificateCheck = project?.omit_certificate_check;

	async function setOmitCertificateCheck(value: boolean | undefined) {
		project.omit_certificate_check = !!value;
		await projectsService.updateProject(project);
	}

	async function setSnapshotLinesThreshold(value: number) {
		project.snapshot_lines_threshold = value;
		await projectsService.updateProject(project);
	}

	async function handleOmitCertificateCheckClick(event: MouseEvent) {
		await setOmitCertificateCheck((event.target as HTMLInputElement)?.checked);
	}
</script>

<Section gap={8}>
	<SectionCard orientation="row" labelFor="omitCertificateCheck">
		{#snippet title()}
			Ignore host certificate checks
		{/snippet}
		{#snippet caption()}
			Enabling this will ignore host certificate checks when authenticating with ssh.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="omitCertificateCheck"
				checked={omitCertificateCheck}
				onclick={handleOmitCertificateCheckClick}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard orientation="row" centerAlign>
		{#snippet title()}
			Snapshot lines threshold
		{/snippet}
		{#snippet caption()}
			The number of lines that trigger a snapshot when saving.
		{/snippet}

		{#snippet actions()}
			<Textbox
				type="number"
				width={100}
				textAlign="center"
				value={snaphotLinesThreshold?.toString()}
				minVal={5}
				maxVal={1000}
				showCountActions
				onchange={(value: string) => {
					setSnapshotLinesThreshold(parseInt(value));
				}}
			/>
		{/snippet}
	</SectionCard>
</Section>
