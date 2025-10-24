<script lang="ts">
	import FeedItem from '$components/FeedItem.svelte';
	import CliSymLink from '$components/profileSettings/CliSymLink.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import { CLI_MANAGER } from '$lib/cli/cli';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { FEED_FACTORY } from '$lib/feed/feed';
	import { useSettingsModal } from '$lib/settings/settingsModal.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Badge, Button, Icon, RichTextEditor, Spacer, Link } from '@gitbutler/ui';
	import { tick } from 'svelte';

	type Props = {
		projectId: string;
		onCloseClick: () => void;
	};

	const { projectId, onCloseClick }: Props = $props();

	const feedFactory = inject(FEED_FACTORY);
	const feed = $derived(feedFactory.getFeed(projectId));
	const actionService = inject(ACTION_SERVICE);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = $derived(settingsService.appSettings);

	const cliManager = inject(CLI_MANAGER);
	const [instalCLI, installingCLI] = cliManager.install;
	const { openProjectSettings } = useSettingsModal();

	const combinedEntries = $derived(feed.combined);
	const lastAddedId = $derived(feed.lastAddedId);

	const [bot, botting] = actionService.bot;

	let viewport = $state<HTMLDivElement>();
	let topSentinel = $state<HTMLDivElement>();
	let bottomAnchor = $state<HTMLDivElement>();
	let canLoadMore = $state(false);
	let prevScrollHeight = $state<number>(0);

	let editor = $state<RichTextEditor>();
	const aiGenEnabled = projectAiGenEnabled(projectId);

	async function sendCommand() {
		const content = await editor?.getPlaintext();
		if (!content || content?.trim() === '') return;
		editor?.clear();
		const [id, messages] = await feed.addUserMessage(content);
		const response = await bot({
			projectId,
			messageId: id,
			chatMessages: messages
		});

		await feed.addAssistantMessage(id, response);
	}

	function handleKeyDown(event: KeyboardEvent | null): boolean {
		if (event === null) return false;

		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			sendCommand();
			return true;
		}
		return false;
	}

	async function loadMoreItems() {
		if (!canLoadMore || !viewport) return;
		canLoadMore = false;
		prevScrollHeight = viewport.scrollHeight;
		await feed.fetch();
		await tick();
		const newScrollHeight = viewport.scrollHeight;
		viewport.scrollTop = newScrollHeight - prevScrollHeight - 5;

		await tick();
		canLoadMore = true;
	}

	$effect(() => {
		if (topSentinel) {
			// Setup observer
			const observer = new IntersectionObserver(
				(entries) => {
					const first = entries[0];
					if (first?.isIntersecting) {
						loadMoreItems();
					}
				},
				{
					root: viewport,
					threshold: 0
				}
			);

			if (topSentinel) {
				observer.observe(topSentinel);
			}

			return () => {
				if (topSentinel) {
					observer.unobserve(topSentinel);
				}
			};
		}
	});

	let showCLISetupSteps = $state(false);
	let showSymlink = $state(false);

	$effect(() => {
		if ($lastAddedId !== null && bottomAnchor) {
			bottomAnchor.scrollIntoView({
				behavior: 'smooth',
				block: 'end'
			});
		}
	});
</script>

<div class="feed-wrap" class:has-actions={$combinedEntries.length > 0}>
	<div class="feed">
		{#if !aiGenEnabled}
			<div class="eneable-ai-banner">
				<Icon name="warning" color="warning" />
				<p class="text-13 text-bold flex-1">Enable AI generation</p>
				<Button
					kind="outline"
					onclick={() => {
						openProjectSettings(projectId, 'ai');
					}}
				>
					Enable in settings
				</Button>
			</div>
		{/if}

		<div class="feed__header">
			<h2 class="flex-1 text-14 text-semibold">Butler Actions</h2>
			<Button icon="cross" kind="ghost" onclick={onCloseClick} />
		</div>
		<div class="feed__scroll-area">
			{#if $combinedEntries.length === 0}
				<div class="feed__empty-state">
					<div class="feed__empty-state__content">
						<div class="feed__empty-state__image">
							{@html laneNewSvg}
						</div>

						<h3 class="text-15 text-bold m-b-12">Welcome to GitButler Actions!</h3>
						<ul class="feed__empty-state__benefits">
							<li class="text-13 text-body">
								<p class="m-b-4">
									<span class="m-r-4">✦</span> Connect your Agentic workflow with GitButler. Editors
									with MCP support (like Cursor, VSCode, Zed) can have changes automatically version
									controlled.
								</p>
								<Link
									href="https://docs.gitbutler.com/features/ai-integration/ai-overview"
									target="_blank"
									class="clr-text-2">Learn more</Link
								>
							</li>
							<li class="text-13 text-body">
								<p class="m-b-6">
									<span class="m-r-4">✷</span>
									Perform automated workflows directly from the app — semantic splitting, amending and
									reorganizing of commits.
								</p>
								<Badge kind="soft" style="pop" size="tag">Coming soon!</Badge>
							</li>
						</ul>

						<Spacer margin={28} />

						<div class="cli-setup">
							<p class="text-13 text-body clr-text-2">
								Using Cursor or other agent-based IDEs? Setup GitButler's MCP integration for
								enhanced workflow automation.
							</p>

							<div
								role="presentation"
								class="cli-setup__fold-btn"
								onclick={() => {
									showCLISetupSteps = !showCLISetupSteps;
								}}
							>
								<div class="cli-setup__fold-icon" class:rotate-icon={showCLISetupSteps}>
									<Icon name="chevron-right" />
								</div>
								<p class="text-15 text-bold underline-dotted">Install GitButler CLI</p>
								<Icon name="robot" />
							</div>

							{#if showCLISetupSteps}
								<ul class="cli-setup__steps text-13 text-body">
									<li>
										<Button
											kind="outline"
											icon="play"
											size="tag"
											loading={installingCLI.current.isLoading}
											onclick={async () => await instalCLI()}>Install But CLI</Button
										>
										<span class="clr-text-2">(requires admin)</span>
										or
										<span
											role="presentation"
											class="underline-dotted"
											onclick={() => {
												showSymlink = !showSymlink;
											}}>configure manually</span
										>
										.
									</li>
								{#if showSymlink}
									<CliSymLink class="m-t-2 m-b-6" />
								{/if}
									<li>
										Cursor / VSCode setup. <Link
											class="clr-text-2"
											href="https://docs.gitbutler.com/features/ai-integration/mcp-server"
											>Learn more</Link
										>
									</li>
								</ul>
							{/if}
						</div>
					</div>
				</div>
			{:else}
				<div class="feed-list" bind:this={viewport}>
					<div bind:this={bottomAnchor} style="height: 1px; margin-top: 8px;"></div>
					{#each $combinedEntries as entry (entry.id)}
						<FeedItem {projectId} action={entry} />
					{/each}
					<div bind:this={topSentinel} style="height: 1px;"></div>
				</div>
			{/if}
		</div>
		{#if $settingsStore?.featureFlags.butbot}
			<div class="feed__input-container">
				<div class="text-input feed__input">
					<RichTextEditor
						bind:this={editor}
						namespace="feed"
						markdown={false}
						styleContext="chat-input"
						placeholder="Tab tab tab"
						disabled={botting.current.isLoading}
						onKeyDown={handleKeyDown}
						onError={(e) => {
							console.error('RichTextEditor error:', e);
						}}
					></RichTextEditor>

					<div class="feed__input-commands">
						<Button
							style="pop"
							icon="arrow-up"
							onclick={sendCommand}
							loading={botting.current.isLoading}
						></Button>
					</div>
				</div>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.feed-wrap {
		display: flex;
		position: relative;
		height: 100%;
		overflow: hidden;
		border-radius: var(--radius-ml);
	}

	.feed {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-1);
	}

	.feed__scroll-area {
		flex: 1;
		min-width: 0px;
		overflow: hidden;
	}

	.feed__header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px 8px 8px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.feed-list {
		box-sizing: border-box;
		display: flex;
		flex-direction: column-reverse;
		height: 100%;
		overflow-x: hidden;
		overflow-y: scroll;
		scrollbar-width: none; /* Firefox */
		-ms-overflow-style: none; /* IE 10+ */

		width: 100%;
		min-width: none;
	}
	.feed-list::-webkit-scrollbar {
		display: none; /* Chrome, Safari, Opera */
	}

	.feed__empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 100%;
		padding: 40px;
		background-color: var(--clr-bg-2);
	}

	.feed__empty-state__content {
		display: flex;
		flex-direction: column;
		max-width: 440px;
		margin-bottom: 40px;
	}

	.feed__empty-state__benefits {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.feed__empty-state__image {
		display: flex;
		margin-bottom: 30px;
	}

	.cli-setup {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.cli-setup__fold-btn {
		display: flex;
		align-items: center;
		margin-left: -4px;
		gap: 8px;
		cursor: pointer;

		&:hover {
			& .cli-setup__fold-icon {
				color: var(--clr-text-1);
			}
		}
	}

	.cli-setup__fold-icon {
		display: flex;
		color: var(--clr-text-2);
		transition:
			transform var(--transition-medium),
			color var(--transition-fast);

		&.rotate-icon {
			transform: rotate(90deg);
		}
	}

	.cli-setup__steps {
		display: flex;
		flex-direction: column;
		margin-left: 20px;
		gap: 6px;
		list-style-type: decimal;
	}

	.eneable-ai-banner {
		display: flex;
		z-index: var(--z-ground);
		position: absolute;
		right: 14px;
		bottom: 14px;
		left: 14px;
		align-items: center;
		justify-content: center;
		padding: 12px;
		padding-left: 16px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-m);
	}
	.feed__input-container {
		display: flex;
		align-items: center;
		padding: 8px;
		border-top: 1px solid var(--clr-border-2);
	}

	.feed__input {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		padding: 4px;
	}

	.feed__input-commands {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
