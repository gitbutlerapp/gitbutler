<script lang="ts">
	import { goto } from '$app/navigation';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FeedItem from '$components/FeedItem.svelte';
	import CliSymLink from '$components/profileSettings/CliSymLink.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import { invoke } from '$lib/backend/ipc';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { persistedChatModelName, projectAiGenEnabled } from '$lib/config/config';
	import { FEED_FACTORY } from '$lib/feed/feed';
	import { newProjectSettingsPath } from '$lib/routes/routes.svelte';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/shared/context';
	import {
		Badge,
		Button,
		Icon,
		RichTextEditor,
		Spacer,
		Link,
		Select,
		SelectItem
	} from '@gitbutler/ui';
	import { onMount, tick } from 'svelte';

	type Props = {
		projectId: string;
		onCloseClick: () => void;
	};

	const { projectId, onCloseClick }: Props = $props();

	const feedFactory = inject(FEED_FACTORY);
	const feed = $derived(feedFactory.getFeed(projectId));
	const actionService = inject(ACTION_SERVICE);
	const user = inject(USER);
	const settingsService = inject(SETTINGS_SERVICE);
	const settingsStore = $derived(settingsService.appSettings);

	const isAdmin = $derived($user.role === 'admin');
	const combinedEntries = $derived(feed.combined);
	const lastAddedId = $derived(feed.lastAddedId);

	const MODELS = ['gpt-4.1', 'gpt-4.1-mini'] as const;

	type Model = (typeof MODELS)[number];
	const RESTRICTED_MODELS: Model[] = ['gpt-4.1'];
	const DEFAULT_MODEL: Model = 'gpt-4.1-mini';

	const selectedModel = persistedChatModelName<Model>(projectId, DEFAULT_MODEL);

	const [freestyle, freestylin] = actionService.freestyle;

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
		const model = isAdmin ? $selectedModel : DEFAULT_MODEL;
		const response = await freestyle({
			projectId,
			messageId: id,
			chatMessages: messages,
			model
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

	onMount(() => {
		if (viewport) {
			setTimeout(() => {
				viewport!.scrollTop = viewport!.scrollHeight;
				canLoadMore = true;
			}, 100);
		}
	});

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
						goto(newProjectSettingsPath(projectId, 'ai'));
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
		<ConfigurableScrollableContainer childrenWrapHeight="100%" bind:viewport>
			{#if $combinedEntries.length === 0}
				<div class="feed__empty-state">
					<div class="feed__empty-state__content">
						<div class="feed__empty-state__image">
							{@html laneNewSvg}
						</div>

						<h3 class="text-15 text-bold m-bottom-12">Welcome to GitButler Actions!</h3>
						<ul class="feed__empty-state__benefits">
							<li class="text-13 text-body">
								<p class="m-bottom-4">
									<span class="m-right-4">✦</span> Connect your Agentic workflow with GitButler. Editors
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
								<p class="m-bottom-6">
									<span class="m-right-4">✷</span>
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
											onclick={async () => await invoke('install_cli')}>Install But CLI</Button
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
										<CliSymLink class="m-top-2 m-bottom-6" />
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
				<div class="feed-list">
					<div bind:this={bottomAnchor} style="height: 1px; margin-top: 8px;"></div>
					{#each $combinedEntries as entry (entry.id)}
						<FeedItem {projectId} action={entry} />
					{/each}
					<div bind:this={topSentinel} style="height: 1px;"></div>
				</div>
			{/if}
		</ConfigurableScrollableContainer>
		{#if $settingsStore?.featureFlags.butbot}
			<div class="feed__input-container">
				<div class="text-input feed__input">
					<RichTextEditor
						bind:this={editor}
						namespace="feed"
						markdown={false}
						styleContext="chat-input"
						placeholder="Tab tab tab"
						disabled={freestylin.current.isLoading}
						onKeyDown={handleKeyDown}
						onError={(e) => {
							console.error('RichTextEditor error:', e);
						}}
					></RichTextEditor>

					<div class="feed__input-commands">
						<Select
							popupAlign="right"
							popupVerticalAlign="top"
							value={$selectedModel}
							maxHeight={200}
							customWidth={150}
							options={MODELS.map((model) => ({
								value: model,
								label: model
							}))}
							onselect={(value: string) => {
								if (value === $selectedModel) return;
								if (!isAdmin) return;
								if (!MODELS.includes(value as Model)) return;
								selectedModel.set(value as Model);
							}}
						>
							{#snippet customSelectButton()}
								<div class="model-selector">
									<span class="text-11 model-selector__selected">{$selectedModel}</span>
									<div class="model-selector__icon"><Icon name="chevron-down-small" /></div>
								</div>
							{/snippet}

							{#snippet itemSnippet({ item, highlighted })}
								{@const disabled = RESTRICTED_MODELS.includes(item.value as Model) && !isAdmin}
								<SelectItem
									selected={item.value === $selectedModel}
									{highlighted}
									{disabled}
									icon={disabled ? 'locked-small' : undefined}
								>
									<p>
										{item.label}
									</p>
								</SelectItem>
							{/snippet}
						</Select>

						<Button
							style="pop"
							icon="arrow-top"
							onclick={sendCommand}
							loading={freestylin.current.isLoading}
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

	.feed__header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px 8px 8px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.feed-list {
		display: flex;
		flex-direction: column-reverse;
		min-height: 100%;
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

	.model-selector {
		display: flex;
		align-items: center;
		padding: 2px 4px 2px 6px;
		gap: 2px;
		color: var(--clr-text-3);
		text-wrap: nowrap;

		&:hover {
			color: var(--clr-text-2);
			& .model-selector__icon {
				color: var(--clr-text-2);
			}
		}
	}

	.model-selector__icon {
		display: flex;
		color: var(--clr-text-3);
		transition: opacity var(--transition-fast);
	}
</style>
