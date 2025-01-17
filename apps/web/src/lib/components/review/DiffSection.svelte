<script lang="ts">
	import { parseDiff, type DiffLineType } from '$lib/diff/parser';
	import type { DiffSection } from '@gitbutler/shared/branches/types';

	interface Props {
		section: DiffSection;
	}

	const { section }: Props = $props();

	const parsedHunks = $derived(parseDiff(section.diffPatch));

	function lineTypeToClass(lineType: DiffLineType): string {
		switch (lineType) {
			case 'add':
				return 'diff-line-added';
			case 'remove':
				return 'diff-line-removed';
			case 'context':
				return 'diff-line-unchanged';
		}
	}
</script>

<div class="diff-section">
	<p class="file-name">{section.newPath}</p>

	{#each parsedHunks as hunk}
		<div class="diff">
			<div class="diff-header">
				<p class="diff-header-text">
					@@ -{hunk.header.oldStart},{hunk.header.oldLength} +{hunk.header.newStart},{hunk.header
						.newLength} @@
				</p>
			</div>
			<div class="diff-content">
				{#each hunk.lines as line}
					<div class={lineTypeToClass(line.type)}>
						<pre><code>{line.line}</code></pre>
					</div>
				{/each}
			</div>
		</div>
	{/each}
</div>

<style lang="postcss">
	.diff-section {
		display: flex;
		padding: 14px;
		flex-direction: column;
		align-items: flex-start;
		gap: 14px;
		align-self: stretch;
	}

	.file-name {
		color: var(--text-1, #1a1614);

		/* base-body/12 */
		font-family: var(--font-family-default, Inter);
		font-size: 12px;
		font-style: normal;
		font-weight: var(--weight-regular, 400);
		line-height: 160%; /* 19.2px */
	}

	.diff {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		align-self: stretch;

		border-radius: var(--m, 6px);
		border: 1px solid var(--diff-count-border, #d4d0ce);
		overflow-x: scroll;

		& pre {
			color: var(--text-1, #1a1614);
			font-family: 'Geist Mono';
			font-size: 12px;
			font-style: normal;
			font-weight: 400;
			line-height: 120%; /* 14.4px */
			padding: 2px 6px;
		}
	}

	.diff-header {
		display: flex;
		padding: 4px 6px;
		align-items: center;
		gap: 10px;
		flex: 1 0 0;
		align-self: stretch;
		border-bottom: 1px solid var(--diff-count-border, #d4d0ce);
		background: var(--bg-1, #fff);
	}

	.diff-header-text {
		color: var(--text-2, #867e79);
		font-family: 'Geist Mono';
		font-size: 12px;
		font-style: normal;
		font-weight: 400;
		line-height: 120%; /* 14.4px */
	}

	.diff-content {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		align-self: stretch;
	}

	.diff-line-added {
		width: 100%;
		background: var(--clr-diff-addition-count-bg);
	}

	.diff-line-removed {
		width: 100%;
		background: var(--clr-diff-deletion-count-bg);
	}
</style>
