<script lang="ts">
	import Spacer from './Spacer.svelte';
	import ClickableCard from '$lib/components/ClickableCard.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import { projectRunCommitHooks } from '$lib/config/config';
	import { createEventDispatcher } from 'svelte';
	import type { Project } from '$lib/backend/projects';

	export let project: Project;

	let allowForcePushing = project?.ok_with_force_push;
	let omitCertificateCheck = project?.omit_certificate_check;

	const runCommitHooks = projectRunCommitHooks(project.id);
	const dispatch = createEventDispatcher<{
		updated: {
			ok_with_force_push?: boolean;
			omit_certificate_check?: boolean;
		};
	}>();

	const onAllowForcePushingChange = () => {
		dispatch('updated', { ok_with_force_push: allowForcePushing });
	};

	const onOmitCertificateCheckChange = () => {
		dispatch('updated', { omit_certificate_check: omitCertificateCheck });
	};

	const onRunCommitHooksChange = () => {
		$runCommitHooks = !$runCommitHooks;
	};
</script>

<section class="wrapper">
	<ClickableCard
		on:click={() => {
			allowForcePushing = !allowForcePushing;
			onAllowForcePushingChange();
		}}
	>
		<svelte:fragment slot="title">Allow force pushing</svelte:fragment>
		<svelte:fragment slot="body">
			Force pushing allows GitButler to override branches even if they were pushed to remote. We
			will never force push to the trunk.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle bind:checked={allowForcePushing} on:change={onAllowForcePushingChange} />
		</svelte:fragment>
	</ClickableCard>

	<ClickableCard
		on:click={() => {
			omitCertificateCheck = !omitCertificateCheck;
			onOmitCertificateCheckChange();
		}}
	>
		<svelte:fragment slot="title">Ignore host certificate checks</svelte:fragment>
		<svelte:fragment slot="body">
			Enabling this will ignore host certificate checks when authenticating with ssh.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle bind:checked={omitCertificateCheck} on:change={onOmitCertificateCheckChange} />
		</svelte:fragment>
	</ClickableCard>

	<ClickableCard on:click={onRunCommitHooksChange}>
		<svelte:fragment slot="title">Run commit hooks</svelte:fragment>
		<svelte:fragment slot="body">
			Enabling this will run any git pre and post commit hooks you have configured in your
			repository.
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<Toggle bind:checked={$runCommitHooks} on:change={onRunCommitHooksChange} />
		</svelte:fragment>
	</ClickableCard>
</section>

<Spacer />

<style lang="post-css">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}
</style>
