<script lang="ts">
	import InfoMessage from '$components/InfoMessage.svelte';
	import { AIService, GitAIConfigKey, KeyOption, type DiffInput } from '$lib/ai/service';
	import { ModelKind, MessageRole, type Prompt } from '$lib/ai/types';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slide } from 'svelte/transition';

	const aiService = getContext(AIService);
	const userService = getContext(UserService);
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
</script>

<div class="ai-credential-check">
	{#if isStreaming || result || error}
		<div transition:slide={{ duration: 250 }}>
			<InfoMessage style={error ? 'warning' : 'success'} filled outlined={false}>
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
							<div class="text-12 text-body error-text">
								<div class="error-header">
									<i class="result-icon">
										<Icon name="error-small" color="error" />
									</i>
									<span>{error}</span>
								</div>
							</div>

							{#if (modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic) && isUsingButlerAPI && !$user}
								<div class="text-12 text-body help-text">
									<span> Please sign in to use GitButler's AI API. </span>
								</div>
							{:else if modelKind === ModelKind.OpenAI || modelKind === ModelKind.Anthropic}
								<div class="text-12 text-body help-text">
									<span> Please check your API key or try GitButler's API. </span>
								</div>
							{:else if modelKind === ModelKind.Ollama}
								<div class="text-12 text-body help-text">
									<span>
										Please check your Ollama endpoint and model configuration.
										<br />
										Make sure Ollama is running locally and accessible.
										<br />
										<Link href="https://ollama.ai">Learn more about Ollama</Link>
									</span>
								</div>
							{:else if modelKind === ModelKind.LMStudio}
								<div class="text-12 text-body help-text">
									<span>
										Please check your LM Studio configuration.
										<br />
										Make sure LM Studio is running locally and accessible.
										<br />
										<Link href="https://lmstudio.ai">Learn more about LM Studio</Link>
									</span>
								</div>
							{/if}
						{:else}
							<div class="text-12 text-body success-text">
								<div class="success-header">
									<i class="result-icon">
										<Icon name={isStreaming ? 'ai' : 'success-small'} color="success" />
									</i>
									<strong>Sample commit message:</strong>
								</div>
								<div class="ai-response">
									<pre class:streaming={isStreaming}>{isStreaming ? streamingResult : result}
										{#if isStreaming}
											<span class="cursor blink">▋</span>
										{/if}
									</pre>
								</div>
							</div>
						{/if}

						{#if showDebug && debugInfo}
							<div class="debug-info text-12">
								<hr />
								<div>Debug info: {debugInfo}</div>
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
		icon={error ? 'error-small' : 'ai'}
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

	<div class="debug-toggle">
		<button class="text-12 debug-button" onclick={toggleDebug}>
			{showDebug ? 'Hide' : 'Show'} Debug Info
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

	.result-icon {
		display: flex;
		align-items: center;
		margin-right: 6px;
	}

	.error-text,
	.success-text {
		display: flex;
		flex-direction: column;
		width: 100%;
	}

	.error-header,
	.success-header {
		display: flex;
		align-items: center;
		margin-bottom: 4px;
	}

	.help-text {
		margin-top: 6px;
		margin-left: 18px;
	}

	.ai-response {
		width: 100%;
		max-height: 150px;
		overflow-y: auto;
		word-break: break-word;
	}

	.ai-response pre {
		box-sizing: border-box;
		width: 100%;
		min-height: 80px;
		margin: 8px 0 0 0;
		padding: 14px 12px;
		border-radius: 4px;
		background-color: var(--clr-bg-1);
		font-size: 12px;
		font-family: var(--font-mono);
		white-space: pre-wrap;
	}

	.ai-response pre.streaming {
		min-height: 80px;
	}

	.cursor {
		display: inline-block;
		color: var(--clr-text-1);
		vertical-align: middle;
	}

	.blink {
		animation: blink 1s step-end infinite;
	}

	@keyframes blink {
		from,
		to {
			opacity: 1;
		}
		50% {
			opacity: 0;
		}
	}

	.debug-toggle {
		display: flex;
		justify-content: flex-end;
	}

	.debug-button {
		padding: 4px 8px;
		border: none;
		background: none;
		color: var(--clr-text-3);
		font-size: 11px;
		text-decoration: underline;
		cursor: pointer;
	}

	.debug-info {
		margin-top: 8px;
		color: var(--clr-text-3);
		font-size: 11px;
		font-family: monospace;
		white-space: pre-wrap;
		word-break: break-word;
	}
</style>
