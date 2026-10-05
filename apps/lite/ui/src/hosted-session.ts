/** The hub's session ends by a form post, which then lands on its sign-in page. */
export const signOut = () => {
	const form = document.createElement("form");
	form.method = "post";
	form.action = "/sign-out";
	document.body.append(form);
	form.submit();
};

/**
 * Ask the hub to send what `from` last published of `branch` to `to`, as if `from` had sent it.
 * The page has no machine of its own, so this is how it sends.
 */
export const sendFromHub = async (send: {
	project: string;
	from: string;
	branch: string;
	to: string;
}): Promise<void> => {
	const response = await fetch("/send", {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(send),
	});
	if (!response.ok) throw new Error((await response.text()) || response.statusText);
};
