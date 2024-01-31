<script lang="ts">
	import FileCardHeader from './FileCardHeader.svelte';
	import HunkViewer from './HunkViewer.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { ContentSection, HunkSection, parseFileSections } from '$lib/utils/fileSections';
	import lscache from 'lscache';
	import { getContext } from 'svelte';
	import { quintOut } from 'svelte/easing';
	import { slide } from 'svelte/transition';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let projectId: string;
	export let branchId: string;
	export let file: File;
	export let conflicted: boolean;
	export let projectPath: string | undefined;
	export let branchController: BranchController;
	export let isUnapplied: boolean;
	export let selectable = false;
	export let selectedOwnership: Writable<Ownership>;

	let rsViewport: HTMLElement;

	const defaultFileWidthRem = persisted<number | undefined>(30, 'defaulFileWidth' + projectId);
	const fileWidthKey = 'fileWidth_';
	let fileWidth: number;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let sections: (HunkSection | ContentSection)[] = [];

	function parseFile(file: File) {
		// When we toggle expansion status on sections we need to assign
		// `sections = sections` to redraw, and why we do not use a reactive
		// variable.
		if (!file.binary && !file.large) sections = parseFileSections(file);
	}
	$: parseFile(file);

	$: maxLineNumber = sections[sections.length - 1]?.maxLineNumber;

	function getGutterMinWidth(max: number) {
		if (max >= 1000) return 2;
		if (max >= 100) return 1.5;
		if (max >= 10) return 1.25;
		return 1;
	}

	$: minWidth = getGutterMinWidth(maxLineNumber);

	$: isFileLocked = sections
		.filter((section): section is HunkSection => section instanceof HunkSection)
		.some((section) => section.hunk.locked);

	function computedAddedRemoved(section: HunkSection | ContentSection): {
		added: any;
		removed: any;
	} {
		if (section instanceof HunkSection) {
			const lines = section.hunk.diff.split('\n');
			return {
				added: lines.filter((l) => l.startsWith('+')).length,
				removed: lines.filter((l) => l.startsWith('-')).length
			};
		}
		return {
			added: 0,
			removed: 0
		};
	}

	fileWidth = lscache.get(fileWidthKey + file.id);
</script>

<div
	class="resize-viewport"
	bind:this={rsViewport}
	in:slide={{ duration: 180, easing: quintOut, axis: 'x' }}
	style:width={`${fileWidth || $defaultFileWidthRem}rem`}
>
	<div id={`file-${file.id}`} class="file-card card">
		<FileCardHeader {file} {isFileLocked} on:close />
		{#if conflicted}
			<div class="mb-2 bg-red-500 px-2 py-0 font-bold text-white">
				<button
					class="font-bold text-white"
					on:click={() => branchController.markResolved(file.path)}
				>
					Mark resolved
				</button>
			</div>
		{/if}

		<ScrollableContainer wide>
			<div class="hunks">
				{#if file.binary}
					Binary content not shown
				{:else if file.large}
					Diff too large to be shown
				{:else}
					{#each sections as section}
						{@const { added, removed } = computedAddedRemoved(section)}
						{#if 'hunk' in section}
							<div class="hunk-wrapper">
								<div class="indicators text-base-11">
									<span class="added">+{added}</span>
									<span class="removed">+{removed}</span>
									{#if section.hunk.locked}
										<div title={section.hunk.lockedTo}>
											<Icon name="locked-small" color="warn" />
										</div>
									{/if}
								</div>
								<HunkViewer
									{file}
									{section}
									{branchId}
									{selectable}
									{isUnapplied}
									{projectPath}
									{selectedOwnership}
									{branchController}
									{isFileLocked}
									{minWidth}
								/>
							</div>
						{/if}
					{/each}
				{/if}
			</div>
		</ScrollableContainer>
	</div>

	<div class="divider-line">
		<Resizer
			viewport={rsViewport}
			direction="right"
			inside
			minWidth={240}
			on:width={(e) => {
				fileWidth = e.detail / (16 * $userSettings.zoom);
				lscache.set(fileWidthKey + file.id, fileWidth, 7 * 1440); // 7 day ttl
				$defaultFileWidthRem = fileWidth;
			}}
		/>
	</div>
</div>

<style lang="postcss">
	.divider-line {
		position: absolute;
		top: 0;
		right: 0;
		width: 1px;
		height: 100%;

		/* background-color: red; */
		/* background-color: var(--clr-theme-container-outline-light); */

		&:after {
			pointer-events: none;
			content: '';
			position: absolute;
			top: 0;
			right: 50%;
			transform: translateX(50%);
			width: 1px;
			height: 100%;
			background-color: var(--clr-theme-container-outline-light);
		}
	}
	.resize-viewport {
		position: relative;
		display: flex;
		height: 100%;
		align-items: self-start;
		overflow: hidden;
		padding: var(--space-12) var(--space-12) var(--space-12) 0;
	}
	.file-card {
		background: var(--clr-theme-container-light);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		max-height: 100%;
		flex-grow: 1;
	}
	.hunks {
		display: flex;
		flex-direction: column;
		position: relative;
		max-height: 100%;
		flex-shrink: 0;
		padding: var(--space-16);
		gap: var(--space-16);
	}
	.hunk-wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-10);
	}
	.indicators {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}
	.added {
		color: #45b156;
	}
	.removed {
		color: #ff3e00;
	}

	@keyframes wiggle {
		0% {
			transform: rotate(0deg);
		}
		40% {
			transform: rotate(0deg);
		}
		60% {
			transform: rotate(2deg);
		}
		80% {
			transform: rotate(-2deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}
	:global(.wiggle) {
		animation: wiggle 0.5s infinite;
	}
</style>
