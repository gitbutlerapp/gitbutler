<script lang="ts">
	import SectionCard from './SectionCard.svelte';
	import Spacer from './Spacer.svelte';
	import { Project } from '$lib/backend/projects';
	import Toggle from '$lib/components/Toggle.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { getContext } from '$lib/utils/context';
	import { createEventDispatcher } from 'svelte';

	const project = getContext(Project);

	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;

	const runCommitHooks = projectRunCommitHooks(project.id);
	const dispatch = createEventDispatcher<{
		updated: {
			ok_with_force_push?: boolean;
			omit_certificate_check?: boolean;
		};
	}>();
</script>

<section class="wrapper">
	<SectionCard orientation="row" labelFor="allowForcePush">
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="caption">
			Force pushing allows GitButler to override branches even if they were pushed to remote. We
			will never force push to the trunk.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="allowForcePush"
				bind:checked={allowForcePushing}
				on:change={() => dispatch('updated', { ok_with_force_push: allowForcePushing })}
			/>
		</svelte:fragment>
	</SectionCard>

	<SectionCard orientation="row" labelFor="omitCertificateCheck">
		<svelte:fragment slot="title">Ignore host certificate checks</svelte:fragment>
		<svelte:fragment slot="caption">
			Enabling this will ignore host certificate checks when authenticating with ssh.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle
				id="omitCertificateCheck"
				bind:checked={omitCertificateCheck}
				on:change={() => dispatch('updated', { omit_certificate_check: omitCertificateCheck })}
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
</section>

<Spacer />

<style lang="post-css">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--size-8);
	}
</style>
