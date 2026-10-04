/** The hub's session ends by a form post, which then lands on its sign-in page. */
export const signOut = () => {
	const form = document.createElement("form");
	form.method = "post";
	form.action = "/sign-out";
	document.body.append(form);
	form.submit();
};
