<script lang="ts">
	import { untrack } from "svelte";
	import type { ScriptStep } from "./terminal-types";

	interface Props {
		height: string;
		script?: ScriptStep[];
		bottomPadding?: string;
		typingSpeed?: number; // Average delay per character in milliseconds
		onComplete?: () => void; // Called when script finishes playing
		onProgress?: (progress: number) => void; // Called with progress 0-1
	}

	const {
		height,
		script = [],
		bottomPadding = "24px",
		typingSpeed = 55,
		onComplete,
		onProgress,
	}: Props = $props();

	// Timing constants for consistent animation
	const LINE_DELAY_MS = 100; // Delay between lines in input
	const INPUT_PAUSE_MS = 400; // Pause after completing input step
	const OUTPUT_PAUSE_MS = 1000; // Pause after output to let users read

	// Detect OS from user agent
	function detectOS(): "macOS" | "Windows" | "Linux" {
		const userAgent = navigator.userAgent.toLowerCase();

		if (userAgent.includes("mac")) {
			return "macOS";
		} else if (userAgent.includes("win")) {
			return "Windows";
		} else {
			return "Linux";
		}
	}

	const os = detectOS();

	let displayedLines = $state<Array<{ text: string; type: "input" | "output" }>>([]);
	let currentStepIndex = 0;
	let currentLineIndex = 0;
	let currentCharIndex = 0;
	let isPlaying = $state(false);
	let timeoutId: ReturnType<typeof setTimeout> | null = null;
	let progressIntervalId: ReturnType<typeof setInterval> | null = null;
	let startTime = 0;
	let estimatedDuration = 0;
	let terminalBodyElement: HTMLDivElement | null = $state(null);

	// Reset and play script whenever it changes
	$effect(() => {
		// Track script changes
		const currentScript = script;

		// Untrack all the state updates that happen during playback
		untrack(() => {
			if (currentScript.length > 0) {
				resetAndPlay();
			}
		});
	});

	// Auto-scroll to bottom when content changes
	$effect(() => {
		// Track displayedLines changes to trigger the effect
		const _currentDisplayedlines = displayedLines;

		// Scroll to bottom
		if (terminalBodyElement) {
			terminalBodyElement.scrollTop = terminalBodyElement.scrollHeight;
		}
	});

	function calculateDuration(script: ScriptStep[]): number {
		let total = 0;
		const avgTypingDelay = typingSpeed; // Use base typing speed for estimation

		script.forEach((step) => {
			if (step.type === "input") {
				step.lines.forEach((line) => {
					total += line.length * avgTypingDelay; // Time to type characters
					total += LINE_DELAY_MS; // Delay between lines
				});
				total += INPUT_PAUSE_MS; // Pause after input step
			} else {
				total += OUTPUT_PAUSE_MS; // Pause after output step
			}
		});

		// Add 4 second delay before switching to next script
		total += 4000;

		return total;
	}

	function startProgressTracking() {
		if (progressIntervalId) {
			clearInterval(progressIntervalId);
		}

		startTime = Date.now();
		estimatedDuration = calculateDuration(script);

		// Update progress smoothly every 50ms
		progressIntervalId = setInterval(() => {
			if (!isPlaying) {
				if (progressIntervalId) {
					clearInterval(progressIntervalId);
					progressIntervalId = null;
				}
				return;
			}

			const elapsed = Date.now() - startTime;
			const progress = Math.min(elapsed / estimatedDuration, 1);
			onProgress?.(progress);
		}, 50);
	}

	function resetAndPlay() {
		// Clear any pending timeouts
		if (timeoutId) {
			clearTimeout(timeoutId);
			timeoutId = null;
		}
		if (progressIntervalId) {
			clearInterval(progressIntervalId);
			progressIntervalId = null;
		}

		displayedLines = [];
		currentStepIndex = 0;
		currentLineIndex = 0;
		currentCharIndex = 0;
		isPlaying = true;
		startProgressTracking();
		playNextStep();
	}

	function playNextStep() {
		if (!isPlaying || currentStepIndex >= script.length) {
			// Keep progress tracking running during the post-script delay to let users focus
			timeoutId = setTimeout(() => {
				isPlaying = false;
				if (progressIntervalId) {
					clearInterval(progressIntervalId);
					progressIntervalId = null;
				}
				onProgress?.(1);
				onComplete?.();
			}, 4000);
			return;
		}

		const step = script[currentStepIndex];

		if (step.type === "output") {
			// Output: show all lines immediately
			step.lines.forEach((line) => {
				displayedLines = [...displayedLines, { text: line, type: "output" }];
			});
			currentStepIndex++;
			timeoutId = setTimeout(() => playNextStep(), OUTPUT_PAUSE_MS); // Pause after output to let users read
		} else {
			// Input: type character by character
			typeNextCharacter();
		}
	}

	function typeNextCharacter() {
		if (!isPlaying) return;

		const step = script[currentStepIndex];
		if (!step || step.type !== "input") return;

		if (currentLineIndex >= step.lines.length) {
			// Finished this input step, move to next
			currentStepIndex++;
			currentLineIndex = 0;
			currentCharIndex = 0;
			timeoutId = setTimeout(() => playNextStep(), INPUT_PAUSE_MS); // Pause after completing input
			return;
		}

		const currentLine = step.lines[currentLineIndex];

		if (currentCharIndex === 0) {
			// Start new line
			displayedLines = [...displayedLines, { text: "", type: "input" }];
		}

		if (currentCharIndex < currentLine.length) {
			// Add next character
			const lastIndex = displayedLines.length - 1;
			const newLines = [...displayedLines];
			newLines[lastIndex] = {
				...newLines[lastIndex],
				text: currentLine.substring(0, currentCharIndex + 1),
			};
			displayedLines = newLines;
			currentCharIndex++;

			// Variable typing speed for more natural feel
			const variance = typingSpeed * 0.4; // 40% variance
			const delay = Math.random() * variance * 2 + (typingSpeed - variance);
			timeoutId = setTimeout(() => typeNextCharacter(), delay);
		} else {
			// Finished this line, move to next
			currentLineIndex++;
			currentCharIndex = 0;
			timeoutId = setTimeout(() => typeNextCharacter(), LINE_DELAY_MS); // Brief pause between lines
		}
	}
</script>

<div
	class="terminal-mockup"
	class:macos={os === "macOS"}
	class:windows={os === "Windows"}
	class:linux={os === "Linux"}
	style="height: {height};"
>
	<div class="terminal-mockup__header">
		{#if os === "macOS"}
			<div class="terminal-mockup__window-controls">
				<img src="/images/cli/mac-window-controls.svg" alt="Window controls" />
			</div>
			<div class="terminal-mockup__title">GitButler CLI</div>
		{:else if os === "Windows"}
			<div class="terminal-mockup__title">GitButler CLI</div>
			<div class="terminal-mockup__window-controls">
				<img src="/images/cli/windows-window-controls.svg" alt="Window controls" />
			</div>
		{:else if os === "Linux"}
			<div class="terminal-mockup__title">GitButler CLI</div>
			<div class="terminal-mockup__window-controls">
				<img src="/images/cli/linux-window-controls.svg" alt="Window controls" />
			</div>
		{/if}
	</div>

	<div
		class="terminal-mockup__body"
		bind:this={terminalBodyElement}
		style:--desktop-padding-bottom={bottomPadding}
	>
		<code class="terminal-mockup__code">
			{#each displayedLines as line, index}
				{#if line.type === "input"}
					<span class="terminal-mockup__line terminal-mockup__input">{@html line.text}</span
					>{#if isPlaying && index === displayedLines.length - 1 && line.type === "input"}<span
							class="terminal-mockup__cursor"
						></span>{/if}
				{:else}
					<span class="terminal-mockup__line terminal-mockup__output">{@html line.text}</span>
				{/if}
				<br />
			{/each}
		</code>
	</div>
</div>

<style>
	.terminal-mockup {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		box-shadow:
			0px 1px 6px rgba(0, 0, 0, 0.1),
			0px 24px 44px 3px rgba(0, 0, 0, 0.2);
		--desktop-padding-bottom: 24px;
	}

	.terminal-mockup.macos {
		border-radius: 20px;

		& .terminal-mockup__header {
			display: flex;
			position: relative;
			padding: 10px 12px;
			background: #ffffff;
		}

		& .terminal-mockup__title {
			position: absolute;
			left: 50%;
			transform: translateX(-50%);
			color: #3c3c4399;
			font-weight: 600;
			font-size: 14px;
			line-height: 1;
			user-select: none;
		}
	}

	.terminal-mockup.windows {
		border-radius: 8px;

		& .terminal-mockup__header {
			display: flex;
			align-items: center;
			justify-content: space-between;
			background: #4b4b46;
		}

		& .terminal-mockup__title {
			margin-top: 8px;
			margin-left: 8px;
			padding: 12px 14px;
			border-top-right-radius: 8px;
			border-top-left-radius: 8px;
			background-color: var(--clr-core-gray-20);
			color: #cccccc;
			font-weight: 400;
			font-size: 14px;
			line-height: 1;
			user-select: none;
		}

		& .terminal-mockup__window-controls {
			margin-right: 18px;
		}
	}

	.terminal-mockup.linux {
		border-radius: 8px;

		& .terminal-mockup__header {
			display: flex;
			align-items: center;
			justify-content: flex-end;
			padding: 8px 12px;
			background: linear-gradient(to bottom, #454545, #2b2b2b);
		}

		& .terminal-mockup__title {
			position: absolute;
			left: 50%;
			transform: translateX(-50%);
			color: #bababf;
			font-weight: 600;
			font-size: 14px;
			line-height: 1;
			user-select: none;
		}
	}

	.terminal-mockup__body {
		flex: 1;
		padding: 24px;
		padding-bottom: var(--desktop-padding-bottom);
		overflow-y: auto;
		background: var(--clr-core-gray-10);
		box-shadow: inset 0px 4px 100px var(--clr-core-gray-20);
		color: #d4d4d4;
		font-size: 14px;
		line-height: 1.5;
		font-family: "Source Code Pro", monospace;
		scrollbar-color: rgba(255, 255, 255, 0.2) transparent;

		/* Custom scrollbar for Firefox */
		scrollbar-width: thin;

		/* Custom scrollbar for Webkit browsers */
		&::-webkit-scrollbar {
			width: 8px;
		}

		&::-webkit-scrollbar-track {
			background: transparent;
		}

		&::-webkit-scrollbar-thumb {
			border-radius: 4px;
			background: rgba(255, 255, 255, 0.2);
		}

		&::-webkit-scrollbar-thumb:hover {
			background: rgba(255, 255, 255, 0.3);
		}
	}

	.terminal-mockup__code {
		display: block;
		white-space: pre-wrap;
	}

	.terminal-mockup__line {
		display: inline;
	}

	.terminal-mockup__input {
		color: #ffffff;
	}

	.terminal-mockup__output {
		color: #d4d4d4;
	}

	.terminal-mockup__cursor {
		display: inline-block;
		width: 0.6em;
		height: 1.2em;
		margin-left: 2px;
		background-color: #ffffff;
		vertical-align: text-bottom;
	}

	/* Utility classes for colored text in terminal output */
	:global(.terminal-mockup__code .t-highlight) {
		color: #ffd700;
		font-weight: 600;
	}

	:global(.terminal-mockup__code .t-teal) {
		color: #4ec9b0;
	}

	:global(.terminal-mockup__code .t-blue) {
		color: #569cd6;
	}

	:global(.terminal-mockup__code .t-green) {
		color: #57a64a;
	}

	:global(.terminal-mockup__code .t-yellow) {
		color: #dcdcaa;
	}

	:global(.terminal-mockup__code .t-success) {
		color: #50fa7b;
	}

	:global(.terminal-mockup__code .t-error) {
		color: #ff5555;
	}

	:global(.terminal-mockup__code .t-warning) {
		color: #ffb86c;
	}

	:global(.terminal-mockup__code .t-dim) {
		color: #8d9cc4;
		opacity: 0.7;
	}

	:global(.terminal-mockup__code .t-accent) {
		color: #bd93f9;
	}

	:global(.terminal-mockup__code .t-cyan) {
		color: #8be9fd;
	}

	@media (--mobile-viewport) {
		.terminal-mockup__body {
			padding-bottom: 24px;
		}
	}
</style>
