import { chipToasts } from '$components/chipToast/chipToastStore';

export function error(msg: string) {
	return chipToasts.error(msg);
}

export function success(msg: string) {
	return chipToasts.success(msg);
}

export function warning(msg: string) {
	return chipToasts.warning(msg);
}

export function neutral(msg: string) {
	return chipToasts.neutral(msg);
}

// Keep loading function for compatibility
export function loading(msg: string) {
	return chipToasts.neutral(msg);
}

// Simple promise function
export async function promise<T>(
	promise: Promise<T>,
	opts: {
		loading: string;
		success: string;
		error: string;
	} = {
		loading: 'Loading...',
		success: 'Success!',
		error: 'Error!'
	}
): Promise<T> {
	const loadingToastId = loading(opts.loading);

	try {
		const result = await promise;
		chipToasts.removeChipToast(loadingToastId);
		success(opts.success);
		return result;
	} catch (err) {
		chipToasts.removeChipToast(loadingToastId);
		error(opts.error);
		throw err;
	}
}

const toasts = { error, success, warning, neutral, loading, promise };
export default toasts;
