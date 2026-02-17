<script lang="ts">
	import SectionHeader from "$home/components/SectionHeader.svelte";

	const features = [
		{ name: "Stacked branch workflows", git: false, gitbutler: true },
		{ name: "Work on multiple branches simultaneously", git: false, gitbutler: true },
		{ name: "Unlimited undo/redo", git: false, gitbutler: true },
		{ name: "Simple, predictable rebasing", git: false, gitbutler: true },
		{ name: "Fast worktree management", git: false, gitbutler: true },
		{ name: "AI agent-friendly commands", git: false, gitbutler: true },
		{ name: "Make commits", git: true, gitbutler: true },
		{ name: "Works with Git repositories", git: true, gitbutler: true },
	];
</script>

<section class="comparison-wrapper">
	<SectionHeader>
		What <i>But gives you</i> over Git
	</SectionHeader>

	<div class="comparison-table">
		<div class="text-16 text-bold table-header">
			<div class="header-cell feature-header">Feature</div>
			<div class="header-cell">Git</div>
			<div class="header-cell">GitButler</div>
		</div>

		{#each features as feature}
			<div class="table-row">
				<div class="text-15 text-semibold feature-cell">{feature.name}</div>
				<div class="status-cell" data-label="Git">
					{#if feature.git}
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M18.75 3.75L8.98106 14.2167C8.58572 14.6403 7.91428 14.6403 7.51895 14.2167L1.25 7.5"
								stroke="#2EB87E"
								stroke-width="1.5"
							/>
						</svg>
					{:else}
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M16 4L10 10M10 10L4 16M10 10L16 16M10 10L4 4"
								stroke="#DE9185"
								stroke-width="1.5"
							/>
						</svg>
					{/if}
				</div>
				<div class="status-cell" data-label="GitButler">
					{#if feature.gitbutler}
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M18.75 3.75L8.98106 14.2167C8.58572 14.6403 7.91428 14.6403 7.51895 14.2167L1.25 7.5"
								stroke="#2EB87E"
								stroke-width="1.5"
							/>
						</svg>
					{:else}
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M16 4L10 10M10 10L4 16M10 10L16 16M10 10L4 4"
								stroke="#DE9185"
								stroke-width="1.5"
							/>
						</svg>
					{/if}
				</div>
			</div>
		{/each}
	</div>

	<div class="comparison-description">
		<p class="text-15 text-body text-center text-balance">
			GitButler works with your existing Git repositories and remotes.
		</p>
		<p class="text-15 text-body text-center text-balance">
			Zero configuration required. It simply layers onto your current workflow.
		</p>
		<svg
			class="description-underline"
			viewBox="0 0 506 77"
			fill="none"
			preserveAspectRatio="none"
			xmlns="http://www.w3.org/2000/svg"
		>
			<path
				d="M505.294 13.3076C329.649 10.9331 175.078 12.8436 1.39733 32.4027C113.825 31.7184 423.089 38.3339 466.162 46.8419C380.602 50.8168 72.0066 56.6063 31.7284 63.8256"
				stroke-width="25"
			/>
		</svg>
	</div>

	<div class="grainy-bg grainy-1"></div>
	<div class="grainy-bg grainy-2"></div>
</section>

<style>
	.comparison-wrapper {
		display: grid;
		position: relative;
		grid-template-columns: subgrid;
		grid-column: full-start / full-end;
		padding: 58px 32px;
		overflow: hidden;
		border-radius: var(--radius-xl);
		background-color: var(--clr-theme-pop-soft);
	}

	.comparison-table {
		z-index: 1;
		position: relative;
		grid-column: narrow-start / narrow-end;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 16px;
		background: var(--clr-bg-1);
	}

	.table-header {
		display: grid;
		grid-template-columns: 2fr 1fr 1fr;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-2);
	}

	.header-cell {
		padding: 20px 24px;
		text-align: left;
	}

	.feature-header {
		text-align: left;
	}

	.table-row {
		display: grid;
		grid-template-columns: 2fr 1fr 1fr;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.table-row:last-child {
		border-bottom: none;
	}

	.feature-cell {
		padding: 20px 24px;
		text-align: left;
	}

	.status-cell {
		display: flex;
		align-items: center;
		padding: 20px 24px;
	}

	.comparison-description {
		z-index: 1;
		position: relative;
		grid-column: narrow-start / narrow-end;
		max-width: 600px;
		margin-top: 32px;
		margin-right: auto;
		margin-left: auto;

		& p {
			z-index: 1;
			position: relative;
		}
	}

	.description-underline {
		z-index: 0;
		position: absolute;
		bottom: -25px;
		left: 0;
		width: 100%;
		height: calc(100% + 30px);
	}

	.description-underline path {
		stroke: #eefda3;
	}

	/* dark theme */
	:global(.dark) .description-underline path {
		stroke: #475d32;
	}

	.grainy-bg {
		opacity: 0.5;
	}

	.grainy-1 {
		top: -30%;
		left: -40%;
	}
	.grainy-2 {
		right: -20%;
		bottom: -70%;
	}

	@media (--mobile-viewport) {
		.comparison-wrapper {
			margin: 0 -16px;
			padding: 32px 16px;
			border-radius: 0;
		}

		.comparison-table {
			border: none;
			border-radius: 0;
			background: none;
		}

		/* Hide the header on mobile */
		.table-header {
			display: none;
		}

		/* Convert rows to card-style blocks */
		.table-row {
			display: block;
			margin-bottom: 6px;
			overflow: hidden;
			border-bottom: none;
			border: 1px solid var(--clr-border-2);
			border-radius: 12px;
		}

		.table-row:last-child {
			margin-bottom: 0;
		}

		/* Feature name spans full width */
		.feature-cell {
			display: block;
			padding: 16px;
			border-bottom: 1px solid var(--clr-border-2);
			background: var(--clr-bg-2);
			font-size: 14px;
		}

		/* Status cells show label and icon side by side */
		.status-cell {
			display: flex;
			align-items: center;
			justify-content: space-between;
			padding: 12px 16px;
			border-bottom: 1px solid var(--clr-border-2);
		}

		.status-cell:last-child {
			border-bottom: none;
		}

		/* Show the data-label attribute before the icon */
		.status-cell::before {
			content: attr(data-label);
			color: var(--clr-text-2);
			font-weight: 600;
			font-size: 13px;
		}

		.status-cell svg {
			width: 18px;
			height: 18px;
		}

		.comparison-description {
			margin-top: 24px;
		}

		.comparison-description p {
			font-size: 13px;
		}

		.description-underline {
			bottom: -20px;
		}
	}
</style>
