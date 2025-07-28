<script lang="ts">
	import InfoMessage from '$components/shared/InfoMessage.svelte';
	import { AI_SERVICE, type DiffInput } from '$lib/ai/service';
	import { ModelKind } from '$lib/ai/types';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { Button, Link } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';

	const aiService = inject(AI_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;

	let testing = $state(false);
	let isStreaming = $state(false);
	let result = $state<string | null>(null);
	let streamingResult = $state<string>('');
	let error = $state<string | null>(null);
	let modelKind = $state<ModelKind | undefined>();
	let isUsingButlerAPI = $state(false);
	let debugInfo = $state<string | null>(null);
	let showDebug = $state(false);
	let showSampleDiff = $state(false);
	let testTimeout: NodeJS.Timeout | null = null;
	let abortController: AbortController | null = null;

	// Simple test diff for commit message generation
	const testDiff: DiffInput[] = [
		{
			filePath: 'example.js',
			diff: `@@ -1,3 +1,5 @@
 function hello() {
  -  return "Hello World";
  +  // Add a greeting with the current time
  +  const now = new Date();
  +  return \`Hello World! The time is \${now.toLocaleTimeString()}\`;
 }`
		}
	];

	async function testAiCredentials() {
		testing = true;
		isStreaming = false;
		result = null;
		streamingResult = '';
		error = null;
		debugInfo = null;

		// Clear any existing timeout
		if (testTimeout) {
			clearTimeout(testTimeout);
			testTimeout = null;
		}

		// Abort any pending request
		if (abortController) {
			abortController.abort();
		}

		// Create a new abort controller for this request
		abortController = new AbortController();

		try {
			// Get current model kind
			modelKind = await aiService.getModelKind();
			debugInfo = `Model kind: ${modelKind}`;

			// Check if using GitButler API
			isUsingButlerAPI = await aiService.usingGitButlerAPI();
			debugInfo += `, Using GB API: ${isUsingButlerAPI}`;

			// Check if configuration is valid
			const isConfigValid = await aiService.validateConfiguration();
			debugInfo += `, Config valid: ${isConfigValid}`;

			if (!isConfigValid) {
				if (modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic) {
					if (isUsingButlerAPI && !$user) {
						throw new Error("Please sign in to use GitButler's AI API");
					} else {
						throw new Error('Please provide a valid API key for your selected AI service');
					}
				} else if (modelKind === ModelKind.Ollama) {
					// Get Ollama configuration for more detailed error
					const endpoint = await aiService.getOllamaEndpoint();
					const model = await aiService.getOllamaModelName();
					throw new Error(
						`Please check Ollama configuration: endpoint=${endpoint}, model=${model}`
					);
				} else if (modelKind === ModelKind.LMStudio) {
					// Get LM Studio configuration for more detailed error
					const endpoint = await aiService.getLMStudioEndpoint();
					throw new Error(`Please check LM Studio configuration: endpoint=${endpoint}`);
				}
			}

			debugInfo += `, Testing commit message generation`;

			// Set a timeout to fail if the streaming doesn't start or complete
			testTimeout = setTimeout(() => {
				if (testing) {
					console.error('AI response timed out after 20 seconds');
					error =
						'AI response timed out after 20 seconds. Please check if your AI service is running properly.';
					testing = false;
					isStreaming = false; // Make sure streaming state is reset on timeout
					debugInfo += `, Timeout after 20s`;

					// Abort the request if possible
					if (abortController) {
						try {
							abortController.abort();
						} catch (err) {
							console.error('Error aborting request:', err);
						}
					}

					// Force a UI update (this ensures the reactive system recognizes the state changes)
					testing = false;
					isStreaming = false;
				}
			}, 20000);

			// Start streaming mode
			isStreaming = true;

			// Use the summarizeCommit method with the onToken callback for streaming
			const aiResult = await aiService.summarizeCommit({
				diffInput: testDiff,
				useEmojiStyle: false,
				useBriefStyle: false,
				onToken: (token) => {
					// Append each token as it comes in
					streamingResult += token;
				}
			});

			// Clear the timeout since we got a result
			if (testTimeout) {
				clearTimeout(testTimeout);
				testTimeout = null;
			}

			// Set the final result (handling undefined case)
			result = aiResult || streamingResult || null;

			debugInfo += `, Received commit message: ${result?.substring(0, 30)}${result && result.length > 30 ? '...' : ''}`;

			// If result is empty or undefined, show an error
			if (!result || result.trim() === '') {
				throw new Error('Received empty response from AI service');
			}
		} catch (e) {
			console.error('AI credential check error:', e);

			// Don't show abort errors as they're expected when we cancel the request
			if (e instanceof Error && e.name === 'AbortError') {
				error = 'AI request was cancelled';
			} else {
				error = e instanceof Error ? e.message : 'Unknown error occurred';
			}

			debugInfo += `, Error: ${error}`;

			// Clear the timeout if there was an error
			if (testTimeout) {
				clearTimeout(testTimeout);
				testTimeout = null;
			}

			// Ensure streaming and testing states are reset on error
			isStreaming = false;
			testing = false;
		} finally {
			testing = false;
			isStreaming = false;
			abortController = null;
		}
	}

	function toggleDebug() {
		showDebug = !showDebug;
	}

	function toggleSampleMessage() {
		showSampleDiff = !showSampleDiff;
	}
</script>

<div class="ai-credential-check">
	{#if isStreaming || result || error}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage
				style={error ? 'error' : 'success'}
				icon={error ? 'error' : isStreaming ? 'robot' : 'success'}
				filled
				outlined={false}
			>
				{#snippet title()}
					{#if error}
						AI credential check failed
					{:else if result}
						AI credential check passed
					{:else if isStreaming}
						AI is responding...
					{/if}
				{/snippet}

				{#snippet content()}
					<div class="result-content" transition:slide={{ duration: 250 }}>
						{#if error}
							{#if (modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic) && isUsingButlerAPI && !$user}
								<span> Please sign in to use GitButler's AI API. </span>
							{:else if modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic}
								<span> Please check your API key or try GitButler's API. </span>
							{:else if modelKind === ModelKind.Ollama}
								<span>
									Please check your Ollama endpoint and model configuration.
									<br />
									Make sure Ollama is running locally and accessible.

									<Link href="https://ollama.ai">Learn more</Link>
								</span>
							{:else if modelKind === ModelKind.LMStudio}
								<span>
									Please check your LM Studio configuration.
									<br />
									Make sure LM Studio is running locally and accessible.

									<Link href="https://lmstudio.ai">Learn more</Link>
								</span>
							{/if}
						{:else}
							<div class="text-12 text-body success-text">
								<h4 class="text-bold">Response:</h4>
								<pre class:streaming={isStreaming}>{isStreaming
										? streamingResult
											? streamingResult.trim()
											: 'Loading...'
										: result?.trim()}
								</pre>
							</div>
						{/if}
					</div>
				{/snippet}
			</InfoMessage>
		</div>
	{/if}
	<Button
		style="pop"
		wide
		icon="ai-small"
		disabled={testing || isStreaming}
		onclick={testAiCredentials}
	>
		{#if testing || isStreaming}
			{isStreaming ? 'AI is responding...' : 'Testing AI connection...'}
		{:else if error}
			Try again
		{:else if result}
			Test again
		{:else}
			Test AI connection
		{/if}
	</Button>

	{#if showDebug && debugInfo}
		<div class="debug-info text-12 text-body">
			<p><span class="text-bold">Debug info</span>:</p>
			<p>{debugInfo}</p>
		</div>
	{/if}

	{#if showSampleDiff}
		<div class="debug-info text-12 text-body">
			<p class="text-bold">Sample diff:</p>
			<pre class="debug-info__code">{testDiff[0]?.diff}</pre>
		</div>
	{/if}

	<div class="debug-info-buttons">
		<button type="button" class="text-12 debug-button" onclick={toggleSampleMessage}>
			{showSampleDiff ? 'Hide' : 'Show'} diff sample
		</button>
		<button
			type="button"
			class="text-12 debug-button"
			class:debug-button_disabled={!debugInfo}
			onclick={toggleDebug}
		>
			{showDebug ? 'Hide' : 'Show'} debug info
		</button>
	</div>
</div>

<style>
	.ai-credential-check {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.result-content {
		display: flex;
		flex-direction: column;
		margin-top: 4px;
		gap: 4px;
	}

	.success-text {
		display: flex;
		flex-direction: column;
		padding: 14px;
		gap: 10px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	/* DEBUG SECTION */
	.debug-button {
		border: none;
		background: none;
		color: var(--clr-text-2);
		font-size: 11px;
		text-decoration: underline dotted;
		cursor: pointer;
	}

	.debug-button_disabled {
		color: var(--clr-text-3);
		cursor: not-allowed;
	}

	.debug-info-buttons {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		width: auto;
		margin-top: 4px;
		gap: 14px;
	}

	.debug-info {
		display: flex;
		flex-direction: column;
		margin-bottom: -8px;
		padding: 14px;
		gap: 4px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.debug-info__code {
		white-space: pre-wrap;
	}
</style>
