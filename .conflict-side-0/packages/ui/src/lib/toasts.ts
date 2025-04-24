import toast, { type ToastOptions, type ToastPosition } from 'svelte-french-toast';

const defaultOptions = {
	position: 'bottom-center' as ToastPosition,
	style: 'border-radius: 8px; background: black; color: #fff; font-size: 0.813em;'
};

export function error(msg: string, options: ToastOptions = {}) {
	return toast.error(msg, { ...defaultOptions, ...options });
}

export function success(msg: string, options: ToastOptions = {}) {
	return toast.success(msg, { ...defaultOptions, ...options });
}

export async function promise(
	promise: Promise<any>,
	opts: { loading: string; success: string; error: string } = {
		loading: 'Loading...',
		success: 'Success!',
		error: 'Error!'
	}
) {
	return await toast.promise(promise, opts, defaultOptions);
}

const toasts = { error, success };
export default toasts;
