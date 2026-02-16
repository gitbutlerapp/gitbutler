<script lang="ts">
	import DiffSection from "$lib/components/review/DiffSection.svelte";
	import { type Section } from "@gitbutler/shared/patches/types";

	interface Props {
		section: Section;
		commitPageHeaderHeight: number;
	}

	const { section, commitPageHeaderHeight }: Props = $props();
</script>

{#if section.sectionType === "diff"}
	<DiffSection {section} {commitPageHeaderHeight} />
{/if}
