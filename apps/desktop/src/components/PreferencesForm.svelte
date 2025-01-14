<script lang="ts">
	import Section from '$components/Section.svelte';
	import { Project, ProjectsService } from '$lib/backend/projects';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const projectsService = getContext(ProjectsService);
	const project = getContext(Project);

	let snaphotLinesThreshold = project?.snapshot_lines_threshold || 20; // when undefined, the default is 20
	let omitCertificateCheck = project?.omit_certificate_check;
	let useNewBranchIntegrationAlgorithm = project?.use_new_branch_integration_algorithm;

	const runCommitHooks = projectRunCommitHooks(project.id);

	async function setOmitCertificateCheck(value: boolean | undefined) {
		project.omit_certificate_check = !!value;
		await projectsService.updateProject(project);
	}

	async function setSnapshotLinesThreshold(value: number) {
		project.snapshot_lines_threshold = value;
		await projectsService.updateProject(project);
	}

	async function setUseNewBranchIntegrationAlgorithm(value: boolean) {
		project.use_new_branch_integration_algorithm = value;
		await projectsService.updateProject(project);
	}

	async function handleOmitCertificateCheckClick(event: MouseEvent) {
		await setOmitCertificateCheck((event.target as HTMLInputElement)?.checked);
	}

	async function handleUseNewBranchIntegrationAlgorithmClick(event: MouseEvent) {
		await setUseNewBranchIntegrationAlgorithm((event.target as HTMLInputElement)?.checked);
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

	<SectionCard orientation="row" labelFor="newBranchIntegrationAlgorithm">
		{#snippet title()}
			Use new branch integration algorithm
		{/snippet}
		{#snippet caption()}
			Enable this to start using the improved way of integrating remote changes into the local
			virtual branches in your workspace.
			<br />
			This does not affect how the target branch is integrated.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="newBranchIntegrationAlgorithm"
				checked={useNewBranchIntegrationAlgorithm}
				onclick={handleUseNewBranchIntegrationAlgorithmClick}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="runHooks" orientation="row">
		{#snippet title()}
			Run commit hooks
		{/snippet}
		{#snippet caption()}
			Enabling this will run any git pre and post commit hooks you have configured in your
			repository.
		{/snippet}
		{#snippet actions()}
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
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
