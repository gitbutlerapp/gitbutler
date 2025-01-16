<script lang="ts">
	import DiffSection from './DiffSection.svelte';
	import { type Section } from '@gitbutler/shared/branches/types';

	interface Props {
		section: Section;
	}

	const { section }: Props = $props();
</script>

{#if section.sectionType === 'diff'}
	<DiffSection {section} />
{/if}
