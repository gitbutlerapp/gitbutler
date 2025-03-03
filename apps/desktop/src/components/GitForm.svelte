<script lang="ts">
	import CommitSigningForm from './CommitSigningForm.svelte';
	import GitForgeForm from '$components/GitForgeForm.svelte';
	import KeysForm from '$components/KeysForm.svelte';
	import Section from '$components/Section.svelte';
	import { platformName } from '$lib/platform/platform';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
</script>

<Section>
	<CommitSigningForm />
	{#if platformName !== 'windows'}
		<Spacer />
		<KeysForm showProjectName={false} />
	{/if}

	<Spacer />
	<GitForgeForm />
</Section>
