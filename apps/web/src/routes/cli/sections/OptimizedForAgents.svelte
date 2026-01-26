<script lang="ts">
	import TerminalMockup from './TerminalMockup.svelte';
	import AiFeaturesJson from '../ai-features.json';
	import Features from '$home/components/Features.svelte';
	import SectionHeader from '$home/components/SectionHeader.svelte';
</script>

<section class="for-agents">
	<SectionHeader>
		Optimized <i>for</i> Coding Agents
	</SectionHeader>
	<p class="text-16 text-body for-agents__description">
		GitButler was designed from the ground up to work seamlessly with AI coding agents.
		<br />
		Predictable commands, structured output, and automation-friendly workflows.
	</p>
	<Features items={AiFeaturesJson} noMargin />
	<div class="for-agents__terminal">
		<TerminalMockup height="300px" />
	</div>
</section>

<style>
	.for-agents {
		display: grid;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
	}

	.for-agents__description {
		grid-column: narrow-start / narrow-end;
		margin-bottom: 32px;
		color: var(--clr-text-2);
	}

	.for-agents__terminal {
		grid-column: narrow-start / narrow-end;
		margin-top: 40px;
	}
</style>
