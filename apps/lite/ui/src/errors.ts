import type { ToastManager, ToastManagerAddOptions } from "@base-ui/react";

export const errorMessageForToast = (error: unknown): string => {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;

	try {
		return JSON.stringify(error);
	} catch {
		return "Unknown error.";
	}
};

/**
 * Raises an error toast whose title says what failed. The raw error is for a bug report rather
 * than for reading, so it sits behind "Copy error message" instead of under the title.
 */
export const addErrorToast = (
	toastManager: Pick<ToastManager, "add" | "update">,
	{ error, ...options }: ToastManagerAddOptions<object> & { error: unknown },
): string => {
	const message = errorMessageForToast(error);
	const copy = () =>
		void navigator.clipboard
			.writeText(message)
			.then(() => toastManager.update(id, { actionProps: { children: "Copied", onClick: copy } }));
	const id = toastManager.add({
		priority: "high",
		...options,
		type: "error",
		actionProps: { children: "Copy error message", onClick: copy },
	});
	return id;
};
