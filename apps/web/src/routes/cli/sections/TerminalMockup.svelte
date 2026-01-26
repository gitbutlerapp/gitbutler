<script lang="ts">
	interface Props {
		height?: string;
	}

	const { height = '400px' }: Props = $props();

	let currentOs = $state('macOS');
</script>

<div
	class="terminal-mockup"
	class:macos={currentOs === 'macOS'}
	class:windows={currentOs === 'Windows'}
	class:linux={currentOs === 'Linux'}
	style="height: {height};"
>
	<div class="terminal-mockup__header">
		<div class="terminal-mockup__window-controls">
			<img src="/images/cli/mac-window-controls.svg" alt="Window controls" />
		</div>
		<div class="terminal-mockup__title">GitButler CLI</div>
	</div>
	<div class="terminal-mockup__body">
		<code class="terminal-mockup__code">
			<span class="terminal-mockup__prompt">$</span> curl -fsSL https://gitbutler.com/install.sh |
			sh
			<br />
			<span class="terminal-mockup__comment"># Installing GitButler CLI...</span>
			<br />
			<span class="terminal-mockup__success">✔ GitButler CLI installed successfully!</span>
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

	.terminal-mockup__body {
		flex: 1;
		padding: 24px;
		background: var(--clr-core-gray-10);
		box-shadow: inset 0px 4px 100px var(--clr-core-gray-20);
		color: #d4d4d4;
		font-size: 14px;
		line-height: 1.5;
		font-family: 'Source Code Pro', monospace;
	}
</style>
