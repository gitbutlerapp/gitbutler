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
	import { Button, Modal, Select, SelectItem, Textbox, TagInput, Toggle } from '@gitbutler/ui';
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
	let tags = $state<Array<{ id: string; label: string }>>([]);
	let tagInputValue = $state('');

	// Private section
	let isPrivate = $state(false);

	// Validate topic input to allow only alphanumeric characters, dashes, and underscores
	function validateTopicInput(value: string): string {
		return value.replace(/[^a-zA-Z0-9-_]/g, '');
	}

	function handleTopicInput(value: string) {
		topicValue = validateTopicInput(value);
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
		tags.forEach((tag) => {
			flags.push({ type: 'hashtag', subject: tag.label });
		});

		// Include private if enabled
		if (isPrivate) {
			flags.push({ type: 'private' });
		}

		return flags;
	}

	function handlePush() {
		const flags = buildGerritFlags();
		onPush(flags);
		modal?.close();
	}

	const canPush = $derived(true);

	export function show() {
		// Reset form state
		status = 'ready';
		topicValue = branchName;
		tags = [];
		tagInputValue = '';
		isPrivate = false;
		modal?.show();
	}

	export function close() {
		modal?.close();
	}
</script>

<Modal bind:this={modal} title="Gerrit push options" width={400} onSubmit={() => handlePush()}>
	<div class="push-options">
		<!-- Status Section -->
		<Select
			label="Status"
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

		<!-- Topic Section -->
		<Textbox
			label="Topic"
			bind:value={topicValue}
			oninput={handleTopicInput}
			placeholder="Enter topic name"
			wide
		/>

		<!-- Tags Section -->
		<TagInput
			label="Tags"
			bind:tags
			bind:value={tagInputValue}
			helperText="Add tags separated by spaces or commas"
			wide
		/>
	</div>

	{#snippet controls(close)}
		<label class="toggle-wrapper">
			<Toggle id="private-toggle" bind:checked={isPrivate} />
			<span class="text-13 text-body clr-text-2">Mark as private 🔒</span>
		</label>
		<div class="flex-1 flex justify-end gap-8">
			<Button kind="outline" onclick={close}>Cancel</Button>
			<Button style="pop" type="submit" disabled={!canPush}>Push</Button>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.push-options {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.toggle-wrapper {
		display: flex;
		align-items: center;
		gap: 10px;

		& label {
			cursor: pointer;
		}
	}
</style>
