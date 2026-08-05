let evaluated = false;

/**
 * Claim the one-per-process right to evaluate the agent setup prompt.
 *
 * The prompt already lives in the root layout, which mounts once per window.
 * This is belt-and-braces against a dev HMR remount showing a second toast.
 */
export function markAgentPromptEvaluated(): boolean {
	if (evaluated) return false;
	evaluated = true;
	return true;
}
