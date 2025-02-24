<script lang="ts">
	import { BUTLER_AI_CLIENT, MessageRole } from '$lib/ai/service';
	import { parseDiffPatchToDiffString } from '$lib/chat/diffPatch';
	import { inject } from '@gitbutler/core/context';

	import { RULES_SERVICE } from '@gitbutler/shared/rules/rulesService';
	import { Button, Modal, ScrollableContainer, Textarea } from '@gitbutler/ui';
	import { tick } from 'svelte';
	import type { ChatMessage } from '@gitbutler/shared/chat/types';
	import type { CreateRuleParams } from '@gitbutler/shared/rules/types';

	type Props = {
		projectSlug: string;
		message: ChatMessage;
	};

	const { message, projectSlug }: Props = $props();

	const diffStringBefore = $derived(parseDiffPatchToDiffString(message.diffPatchArray, 'before'));
	const diffStringAfter = $derived(parseDiffPatchToDiffString(message.diffPatchArray, 'after'));
	const fileExtension = $derived(message.diffPath?.split('.').pop() ?? '');
	const aiService = inject(BUTLER_AI_CLIENT);
	const rulesService = inject(RULES_SERVICE);

	let isGenerating = $state<boolean>(false);

	let modal = $state<Modal>();
	let ruleTitle = $state<string>();
	let ruleDescription = $state<string>();

	let ruleNegativeExample = $state<string>();
	const effectiveNegativeExample = $derived(ruleNegativeExample ?? diffStringAfter);

	let rulePositiveExample = $state<string>();
	const effectivePositiveExample = $derived(rulePositiveExample ?? diffStringBefore);

	const shouldShowExample = $derived(
		message.diffPatchArray !== undefined && message.diffPatchArray.length > 0
	);

	const SYSTEM_PROMPT = $derived(`
		You're an expert code generation rule creator.
		Take the user's input and help them create rule metadata for it.
		Respond with the actual values ONLY, skip additional information.
		`);

	const diffPatchContext = $derived(
		shouldShowExample
			? `
		- Diff patch:
			\`\`\`${fileExtension}
			${diffStringAfter}
			\`\`\`
	`
			: ''
	);

	const TITLE_PROMPT = $derived(
		`Please, create a title for the rule based on the given content. Return only the actual title.
		
		Use the following information to create the rule metadata:
		- Comment:
			${message.comment}
		${diffPatchContext}
		`
	);

	const DESCTIPTION_PROMPT = $derived(
		`Please, create a description for the rule based on the given content. Return only the actual description. Be super detailed and specific.
		
		Use the following information to create the rule metadata:
		- Comment:
			${message.comment}
		${diffPatchContext}
		`
	);

	const NEGATIVE_EXAMPLE_PROMPT = $derived(
		`Please, create an example of what NOT to do based on the given content. Return only the actual code text content.
		
		Use the following information to create the rule metadata:
		- Comment:
			${message.comment}
		${diffPatchContext}
		`
	);

	const POSITIVE_EXAMPLE_PROMPT = $derived(
		`Please, create an example of what to do based on the given content. Return only the actual code text content.
		
		Use the following information to create the rule metadata:
		- Comment:
			${message.comment}
		${diffPatchContext}
		`
	);

	function cleanCodeGenerationText(text: string): string {
		return text
			.replace(/^\s*```[a-zA-Z0-9]*\n/, '')
			.replace(/\n```$/, '')
			.trim();
	}

	async function kickOffGeneration() {
		if (isGenerating) {
			return;
		}

		isGenerating = true;

		ruleTitle = '';
		ruleDescription = '';

		await tick();
		const titlePromise = aiService.evaluate(
			SYSTEM_PROMPT,
			[{ content: TITLE_PROMPT, role: MessageRole.User }],
			(token: string) => {
				ruleTitle += token;
			}
		);
		const descriptionPromise = aiService.evaluate(
			SYSTEM_PROMPT,
			[{ content: DESCTIPTION_PROMPT, role: MessageRole.User }],
			(token: string) => {
				ruleDescription += token;
			}
		);

		await Promise.all([titlePromise, descriptionPromise]);

		if (shouldShowExample) {
			rulePositiveExample = '';
			ruleNegativeExample = '';
			await tick();

			const negativeExamplePrompt = NEGATIVE_EXAMPLE_PROMPT + '\n' + ruleDescription;
			const positiveExamplePrompt = POSITIVE_EXAMPLE_PROMPT + '\n' + ruleDescription;
			const negativeExamplePromise = aiService
				.evaluate(
					SYSTEM_PROMPT,
					[{ content: negativeExamplePrompt, role: MessageRole.User }],
					(token: string) => {
						ruleNegativeExample += token;
					}
				)
				.then((result) => {
					ruleNegativeExample = cleanCodeGenerationText(result);
				});
			const positiveExamplePromise = aiService
				.evaluate(
					SYSTEM_PROMPT,
					[{ content: positiveExamplePrompt, role: MessageRole.User }],
					(token: string) => {
						rulePositiveExample += token;
					}
				)
				.then((result) => {
					rulePositiveExample = cleanCodeGenerationText(result);
				});
			await Promise.all([negativeExamplePromise, positiveExamplePromise]);
		}

		await tick();
		isGenerating = false;
	}

	function validateInputs(): CreateRuleParams | undefined {
		if (
			!ruleTitle ||
			ruleTitle.trim() === '' ||
			!ruleDescription ||
			ruleDescription.trim() === ''
		) {
			return undefined;
		}

		if (shouldShowExample) {
			if (!ruleNegativeExample || !rulePositiveExample) {
				return undefined;
			}
		}

		return {
			projectSlug,
			title: ruleTitle.trim(),
			description: ruleDescription.trim(),
			negativeExample: effectiveNegativeExample,
			positiveExample: effectivePositiveExample
		};
	}

	async function createRule(close: () => void) {
		const validatedInputs = validateInputs();
		if (!validatedInputs) {
			return;
		}

		await rulesService.createRule(validatedInputs);
		close();
	}

	export function show() {
		kickOffGeneration();
		modal?.show();
	}
</script>

{#snippet titleInput()}
	<div class="rules-modal__input text-input" class:disabled={isGenerating}>
		<Textarea
			value={ruleTitle}
			unstyled
			placeholder="Rule title"
			fontSize={13}
			fontWeight="semibold"
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			disabled={isGenerating}
			spellcheck={false}
			flex="1"
			minRows={1}
			maxRows={10}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleTitle = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet descriptionInput()}
	<div class="rules-modal__input text-input" class:disabled={isGenerating}>
		<Textarea
			value={ruleDescription}
			unstyled
			placeholder="Rule description"
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			disabled={isGenerating}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleDescription = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet negativeExampleInput()}
	<p>Don't do this</p>
	<div class="rules-modal__input text-input" class:disabled={isGenerating}>
		<Textarea
			value={effectiveNegativeExample}
			unstyled
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			disabled={isGenerating}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleNegativeExample = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet positiveExampleInput()}
	<p>Do this</p>
	<div class="rules-modal__input text-input" class:disabled={isGenerating}>
		<Textarea
			value={effectivePositiveExample}
			unstyled
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			disabled={isGenerating}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				rulePositiveExample = target.value;
			}}
		/>
	</div>
{/snippet}

<Modal bind:this={modal} title="Create a rule" onSubmit={createRule}>
	<div class="rules-modal-wrapper">
		<ScrollableContainer whenToShow="hover">
			<div class="rules-modal">
				<p class="text-16">Enter the information about the rule that should be created</p>
				{@render titleInput()}
				{@render descriptionInput()}
				{#if shouldShowExample}
					<p class="text-14">Examples</p>
					{@render negativeExampleInput()}
					{@render positiveExampleInput()}
				{/if}
			</div>
		</ScrollableContainer>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" loading={isGenerating}>Create rule</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.rules-modal-wrapper {
		display: flex;
		flex-direction: column;
		max-height: 520px;
	}
	.rules-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.rules-modal__input {
		display: flex;
		flex-direction: column;

		width: 100%;
		gap: 8px;

		&.disabled {
			opacity: 0.5;
			pointer-events: none;
		}
	}
</style>
