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
	let useNewLocking = $state(project?.use_experimental_locking || false);

	const runCommitHooks = projectRunCommitHooks(project.id);

	async function setOmitCertificateCheck(value: boolean | undefined) {
		project.omit_certificate_check = !!value;
		await projectsService.updateProject(project);
	}

	async function setSnapshotLinesThreshold(value: number) {
		project.snapshot_lines_threshold = value;
		await projectsService.updateProject(project);
	}

	async function setUseNewLocking(value: boolean) {
		project.use_experimental_locking = value;
		await projectsService.updateProject(project);
	}

	$effect(() => {
		setUseNewLocking(useNewLocking);
	});

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

	<SectionCard labelFor="useNewLocking" orientation="row">
		{#snippet title()}
			Use new experimental hunk locking algorithm
		{/snippet}
		{#snippet caption()}
			This new hunk locking algorithm is still in the testing phase but should more accurately catch
			locks and subsequently cause fewer errors.
		{/snippet}
		{#snippet actions()}
			<Toggle id="useNewLocking" bind:checked={useNewLocking} />
		{/snippet}
	</SectionCard>
</Section>
