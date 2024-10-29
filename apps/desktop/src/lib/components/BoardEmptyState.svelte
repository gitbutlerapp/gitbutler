<script lang="ts">
	import zenSvg from '$lib/assets/dzen-pc.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getEditorUri, openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { getContext, getContextStore, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { Writable } from 'svelte/store';

	const gitHost = getGitHost();
	const baseBranch = getContextStore(BaseBranch);
	const branchController = getContext(BranchController);
	const userSettings = getContextStoreBySymbol<Settings, Writable<Settings>>(SETTINGS);

	const project = getContext(Project);

	async function openInEditor() {
		const path = getEditorUri({
			schemeId: $userSettings.defaultCodeEditor.schemeIdentifer,
			path: [project.vscodePath],
			searchParams: { windowId: '_blank' }
		});
		openExternalUrl(path);
	}
</script>

<div data-tauri-drag-region class="empty-board__wrapper transition-fly">
	<div class="empty-board">
		<div class="empty-board__content">
			<div class="empty-board__about">
				<h3 class="text-serif-40 text-body">You're up to date</h3>
				<p class="text-14 text-body">
					Your working directory matches the base branch.
					<br />
					Any edits auto-create a virtual branch for easy management.
				</p>
			</div>

			<div class="empty-board__suggestions">
				<div class="empty-board__suggestions__block">
					<h3 class="text-14 text-bold">Start</h3>
					<div class="empty-board__suggestions__links">
						<div
							class="empty-board__suggestions__link"
							role="button"
							tabindex="0"
							on:keypress={async () => await branchController.createBranch({})}
							on:click={async () => await branchController.createBranch({})}
						>
							<div class="empty-board__suggestions__link__icon">
								<Icon name="new-branch" />
							</div>
							<span class="text-12">Create a new branch</span>
						</div>
						<button
							type="button"
							class="empty-board__suggestions__link"
							on:click={async () => await openExternalUrl('https://docs.gitbutler.com')}
						>
							<div class="empty-board__suggestions__link__icon">
								<Icon name="docs" />
							</div>

							<span class="text-12">GitButler Docs</span>
						</button>
						<button
							type="button"
							class="empty-board__suggestions__link"
							on:keypress={async () => await openInEditor()}
							on:click={async () => await openInEditor()}
						>
							<div class="empty-board__suggestions__link__icon">
								<Icon name="vscode" />
							</div>
							<span class="text-12">`Open in {$userSettings.defaultCodeEditor.displayName}`</span>
						</button>
					</div>
				</div>

				<div class="empty-board__suggestions__block">
					<h3 class="text-14 text-bold">Recent commits</h3>
					<div class="empty-board__suggestions__links">
						{#each ($baseBranch?.recentCommits || []).slice(0, 4) as commit}
							<a
								class="empty-board__suggestions__link"
								href={$gitHost?.commitUrl(commit.id)}
								target="_blank"
								rel="noreferrer"
								title="Open in browser"
							>
								<div class="empty-board__suggestions__link__icon">
									<Icon name="commit" />
								</div>

								<span class="text-12">{commit.description}</span>
							</a>
						{/each}
					</div>
				</div>
			</div>
		</div>

		<div data-tauri-drag-region class="empty-board__image-frame">
			<div class="empty-board__image">
				{@html zenSvg}
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.empty-board__wrapper {
		display: flex;
		justify-content: center;
		align-items: center;
		height: 100%;
		width: 100%;
		padding: 0 40px;
	}

	.empty-board {
		display: flex;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		width: 100%;
		gap: 48px;
		max-width: 736px;
		min-height: 320px;
		padding: 32px;
	}

	.empty-board__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		padding-left: 4px;
	}

	.empty-board__image-frame {
		flex-shrink: 0;
		position: relative;
		width: 180px;
		height: auto;
		border-radius: var(--radius-l);
		background-color: var(--clr-illustration-bg);

		&::before {
			content: '';
			display: block;
			position: absolute;
			bottom: 12%;
			left: 50%;
			width: 104px;
			height: 24px;
			transform: translateX(-50%) scale(1.15);
			border-radius: 100%;
			background-color: var(--clr-illustration-outline);
			opacity: 0.09;
			animation: shadow-scale 5.5s infinite ease-in-out;
			animation-delay: 3s;
		}
	}

	.empty-board__image {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -70%) translateZ(0);
		width: 212px;
		animation: hovering 5.5s infinite ease-in-out;
		animation-delay: 3s;
	}
	@keyframes hovering {
		0% {
			transform: translate(-50%, -70%) translateZ(0);
		}
		50% {
			transform: translate(-50%, -65%) translateZ(0);
		}
		100% {
			transform: translate(-50%, -70%) translateZ(0);
		}
	}
	@keyframes shadow-scale {
		0% {
			opacity: 0.09;
			transform: translateX(-50%) scale(1.15);
		}
		50% {
			opacity: 0.12;
			transform: translateX(-50%) scale(1);
		}
		100% {
			opacity: 0.09;
			transform: translateX(-50%) scale(1.15);
		}
	}

	.empty-board__about {
		display: flex;
		flex-direction: column;
		margin-bottom: 32px;
	}

	.empty-board__about h3 {
		color: var(--clr-scale-ntrl-0);
	}

	.empty-board__about p {
		color: var(--clr-scale-ntrl-40);
		line-height: 160%;
	}

	.empty-board__suggestions {
		display: flex;
		flex-direction: row;
		gap: 40px;
	}

	.empty-board__suggestions__block {
		display: flex;
		flex-direction: column;
		gap: 16px;
		min-width: 160px;
	}

	.empty-board__suggestions__block h3 {
		color: var(--clr-scale-ntrl-0);
	}

	.empty-board__suggestions__links {
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-left: -4px;
	}

	.empty-board__suggestions__link {
		cursor: pointer;
		display: flex;
		align-items: center;
		width: fit-content;
		max-width: 100%;
		padding: 6px;
		border-radius: var(--radius-s);
		gap: 10px;
		transition: background-color var(--transition-fast);
		overflow: hidden;

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		& span {
			color: var(--clr-scale-ntrl-40);
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	.empty-board__suggestions__link__icon {
		display: flex;
		color: var(--clr-scale-ntrl-50);
	}
</style>
