<script lang="ts">
	import { Project, ProjectsService } from '$lib/backend/projects';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import { getContext } from '@gitbutler/shared/context';
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
		<svelte:fragment slot="title">Ignore host certificate checks</svelte:fragment>
		<svelte:fragment slot="caption">
			Enabling this will ignore host certificate checks when authenticating with ssh.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="omitCertificateCheck"
				checked={omitCertificateCheck}
				onclick={handleOmitCertificateCheckClick}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" labelFor="newBranchIntegrationAlgorithm">
		<svelte:fragment slot="title">Use new branch integration algorithm</svelte:fragment>
		<svelte:fragment slot="caption"
			>Enable this to start using the improved way of integrating remote changes into the local
			virtual branches in your workspace.
			<br />
			This does not affect how the target branch is integrated.</svelte:fragment
		>
		<svelte:fragment slot="actions">
			<Toggle
				id="newBranchIntegrationAlgorithm"
				checked={useNewBranchIntegrationAlgorithm}
				onclick={handleUseNewBranchIntegrationAlgorithmClick}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard labelFor="runHooks" orientation="row">
		<svelte:fragment slot="title">Run commit hooks</svelte:fragment>
		<svelte:fragment slot="caption">
			Enabling this will run any git pre and post commit hooks you have configured in your
			repository.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle id="runHooks" bind:checked={$runCommitHooks} />
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" centerAlign>
		<svelte:fragment slot="title">Snapshot lines threshold</svelte:fragment>
		<svelte:fragment slot="caption">
			The number of lines that trigger a snapshot when saving.
		</svelte:fragment>

		<svelte:fragment slot="actions">
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
		</svelte:fragment>
	</SectionCard>
</Section>
