<script lang="ts" module>
	export type GerritPushModalProps = {
		projectId: string;
		stackId?: string;
		branchName: string;
		multipleBranches: boolean;
		isLastBranchInStack?: boolean;
		isFirstBranchInStack?: boolean;
		onPush: (gerritFlags: import('$lib/stacks/stack').GerritPushFlag[]) => void;
	};
</script>

<script lang="ts">
	import { Button, Modal, SectionCard, Select, SelectItem, Textbox, Toggle } from '@gitbutler/ui';
	import type { GerritPushFlag } from '$lib/stacks/stack';

	const {
		projectId: _projectId,
		stackId: _stackId,
		branchName,
		multipleBranches: _multipleBranches,
		isLastBranchInStack: _isLastBranchInStack,
		isFirstBranchInStack: _isFirstBranchInStack,
		onPush
	}: GerritPushModalProps = $props();

	let modal = $state<Modal>();

	// Status section - WIP or Ready (default Ready)
	let status = $state<'wip' | 'ready'>('ready');

	// Topic section
	let topicValue = $state(branchName);

	// Tags section
	let committedTags = $state<string[]>([]);
	let currentTagInput = $state('');

	// Private section
	let isPrivate = $state(false);

	// Commit current input as a tag
	function commitCurrentTag() {
		const trimmed = validateTagInput(currentTagInput.trim());
		if (trimmed && !committedTags.includes(trimmed)) {
			committedTags = [...committedTags, trimmed];
			currentTagInput = '';
		}
	}

	// Handle keyboard input for tags
	function handleTagKeydown(event: KeyboardEvent) {
		if (event.key === ' ' || event.key === 'Enter' || event.key === ',') {
			event.preventDefault();
			commitCurrentTag();
		} else if (event.key === 'Backspace' && currentTagInput === '' && committedTags.length > 0) {
			// Remove last tag if backspace pressed on empty input
			committedTags = committedTags.slice(0, -1);
		}
	}

	// Validate topic input to allow only alphanumeric characters, dashes, and underscores
	function validateTopicInput(value: string): string {
		return value.replace(/[^a-zA-Z0-9-_]/g, '');
	}

	// Validate tag input to allow alphanumeric characters, dashes, and underscores
	function validateTagInput(value: string): string {
		return value.replace(/[^a-zA-Z0-9-_]/g, '');
	}

	function handleTopicInput(value: string) {
		topicValue = validateTopicInput(value);
	}

	function handleTagInput(event: Event) {
		const target = event.target as HTMLInputElement;
		currentTagInput = target.value;
	}

	function buildGerritFlags(): GerritPushFlag[] {
		const flags: GerritPushFlag[] = [];

		// Always include status (wip or ready)
		flags.push({ type: status });

		// Include topic if has value
		if (topicValue.trim()) {
			flags.push({ type: 'topic', subject: topicValue.trim() });
		}

		// Include hashtags if has values
		committedTags.forEach((tag) => {
			flags.push({ type: 'hashtag', subject: tag });
		});

		// Include private if enabled
		if (isPrivate) {
			flags.push({ type: 'private' });
		}

		return flags;
	}

	function handlePush() {
		// Commit any remaining input as a tag before building flags
		commitCurrentTag();
		const flags = buildGerritFlags();
		onPush(flags);
		modal?.close();
	}

	const canPush = $derived(true);

	export function show() {
		// Reset form state
		status = 'ready';
		topicValue = branchName;
		committedTags = [];
		currentTagInput = '';
		isPrivate = false;
		modal?.show();
	}

	export function close() {
		modal?.close();
	}
</script>

<Modal bind:this={modal} title="Gerrit Push Options" width="medium" onSubmit={() => handlePush()}>
	<div class="gerrit-push-modal">
		<div class="push-options">
			<!-- Status Section -->
			<SectionCard orientation="row" centerAlign>
				{#snippet title()}
					Status
				{/snippet}
				{#snippet actions()}
					<Select
						value={status}
						options={[
							{ label: 'Ready for review', value: 'ready' },
							{ label: 'Work in progress', value: 'wip' }
						]}
						onselect={(value) => {
							status = value as 'ready' | 'wip';
						}}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === status} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				{/snippet}
			</SectionCard>

			<!-- Topic Section -->
			<SectionCard orientation="row" centerAlign>
				{#snippet title()}
					Topic
				{/snippet}
				{#snippet actions()}
					<div class="topic-input-container">
						<Textbox
							bind:value={topicValue}
							oninput={handleTopicInput}
							placeholder="Enter topic name"
							wide
						/>
					</div>
				{/snippet}
			</SectionCard>

			<!-- Tags Section -->
			<SectionCard orientation="row" centerAlign>
				{#snippet title()}
					Tags
				{/snippet}
				{#snippet actions()}
					<div class="tags-input-container">
						<div
							class="tags-input-field"
							role="textbox"
							tabindex="0"
							onclick={(e) => {
								const input = e.currentTarget.querySelector('.tags-input') as HTMLInputElement;
								if (input) input.focus();
							}}
							onkeydown={(e) => {
								if (e.key === 'Enter' || e.key === ' ') {
									const input = e.currentTarget.querySelector('.tags-input') as HTMLInputElement;
									if (input) input.focus();
								}
							}}
						>
							{#each committedTags as tag}
								<span class="tag-pill-inline">{tag}</span>
							{/each}
							<input
								class="tags-input"
								type="text"
								bind:value={currentTagInput}
								oninput={handleTagInput}
								onkeydown={handleTagKeydown}
								onblur={commitCurrentTag}
								placeholder={committedTags.length === 0 && currentTagInput === ''
									? 'Enter tags (space or comma to separate)'
									: ''}
							/>
						</div>
					</div>
				{/snippet}
			</SectionCard>

			<!-- Private Section -->
			<SectionCard labelFor="private-toggle" orientation="row">
				{#snippet title()}
					Mark as private
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="private-toggle"
						checked={isPrivate}
						onclick={() => (isPrivate = !isPrivate)}
					/>
				{/snippet}
			</SectionCard>
		</div>
	</div>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" disabled={!canPush}>Push</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.gerrit-push-modal {
		display: flex;
		flex-direction: column;
		margin-top: 8px;
		gap: 16px;
	}

	.description {
		margin: 0;
		line-height: 1.5;
	}

	.push-options {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.topic-input-container {
		flex-shrink: 0;
		width: 320px;
	}

	.tags-input-container {
		flex-shrink: 0;
		width: 320px;
	}

	.tags-input-field {
		display: flex;
		flex-wrap: nowrap;
		align-items: center;
		height: 32px;
		min-height: 32px;
		padding: 5px 8px;
		overflow-x: auto;
		overflow-y: hidden;
		gap: 3px;
		border: 1px solid var(--clr-border-2);
		border-radius: 6px;
		background: var(--clr-bg-1);
		cursor: text;
	}

	.tags-input-field:hover {
		border-color: var(--clr-border-3);
	}

	.tags-input-field:focus-within {
		border-color: var(--clr-border-2);
		box-shadow: var(--focus-box-shadow);
	}

	.tag-pill-inline {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		height: 18px;
		padding: 1px 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: 10px;
		background: var(--clr-bg-2);
		color: var(--clr-text-2);
		font-size: 11px;
		line-height: 1.2;
		white-space: nowrap;
	}

	.tags-input {
		flex: 1;
		flex-shrink: 0;
		min-width: 120px;
		border: none;
		outline: none;
		background: transparent;
		color: var(--clr-text-1);
		font-size: 13px;
	}

	.tags-input::placeholder {
		color: var(--clr-text-3);
	}
</style>
