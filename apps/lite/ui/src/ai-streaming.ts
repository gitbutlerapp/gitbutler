/**
 * Driving a streamed AI response into a form field.
 *
 * Shared by every generator (commit messages, PR descriptions) rather than
 * owned by one, because they all face the same problem: a stream that dies
 * halfway has already overwritten what the user wrote.
 */

/**
 * Streams partial values, and calls `onFailure` to put the field back if a
 * stream dies once it has already overwritten something.
 *
 * Restoring is a callback rather than a value this could write back through
 * `onValue`, because a caller may split the stream across several fields —
 * there is then no single value whose round trip reproduces what was there.
 */
export const streamGeneratedText = async (
	stream: (onToken: (token: string) => void) => Promise<string>,
	onValue: (value: string) => void,
	onFailure: () => void,
): Promise<string> => {
	let partial = "";
	try {
		const response = await stream((token) => {
			partial += token;
			onValue(partial);
		});
		onValue(response);
		return response;
	} catch (error) {
		if (partial.length > 0) onFailure();
		throw error;
	}
};
