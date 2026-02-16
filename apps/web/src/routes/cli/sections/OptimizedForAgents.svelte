<script lang="ts">
	import TerminalMockup from "./TerminalMockup.svelte";
	import AiFeaturesJson from "../ai-features.json";
	import Features from "$home/components/Features.svelte";
	import SectionHeader from "$home/components/SectionHeader.svelte";
	import type { ScriptStep } from "./terminal-types";

	const script: ScriptStep[] = [
		{
			type: "input",
			lines: ["<span class='t-dim'># AI can automate branching workflows</span>"],
		},
		{
			type: "input",
			lines: ["$ but branch new feature/ai-integration"],
		},
		{
			type: "output",
			lines: [
				"<span class='t-success'>✓</span> Created branch <span class='t-highlight'>feature/ai-integration</span>",
				"",
			],
		},
		{
			type: "input",
			lines: ["$ but commit -m 'feat: Add AI-powered suggestions'"],
		},
		{
			type: "output",
			lines: [
				"<span class='t-success'>✓</span> Created commit <span class='t-accent'>abc123d</span>",
				"",
			],
		},
		{
			type: "input",
			lines: ["$ but status --format=json | jq ."],
		},
		{
			type: "output",
			lines: [
				'<span class="t-dim">{</span><span class="t-cyan">"branches"</span><span class="t-dim">:</span> <span class="t-dim">[{</span><span class="t-cyan">"name"</span><span class="t-dim">:</span> <span class="t-highlight">"feature/ai-integration"</span><span class="t-dim">,</span> <span class="t-cyan">"commits"</span><span class="t-dim">:</span> <span class="t-accent">1</span><span class="t-dim">}]}</span>',
			],
		},
	];
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
		<TerminalMockup height="300px" {script} typingSpeed={40} />
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
