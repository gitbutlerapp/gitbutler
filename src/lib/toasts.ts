import toast, { type ToastOptions, type ToastPosition } from 'svelte-french-toast';

const defaultOptions = {
	position: 'bottom-center' as ToastPosition,
	style: 'border-radius: 8px; background: black; color: #fff;'
};

export function error(msg: string, options: ToastOptions = {}) {
	return toast.error(msg, { ...defaultOptions, ...options });
}

export function success(msg: string, options: ToastOptions = {}) {
	return toast.success(msg, { ...defaultOptions, ...options });
}

export function promise(
	promise: Promise<any>,
	opts: { loading: string; success: string; error: string } = {
		loading: 'Loading...',
		success: 'Success!',
		error: 'Error!'
	}
) {
	return toast.promise(promise, opts, defaultOptions);
}
